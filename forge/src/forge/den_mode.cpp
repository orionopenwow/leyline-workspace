// This file is part of Forge, based on Noggit Red; licensed under GNU General Public License (version 3).
// Forge change (not in Noggit Red): Den mode. See docs/DEN-MODE.md and CHANGES-FROM-NOGGIT-RED.md.
//
// Den mode opens the editor straight on one Den's map tile, keeps every write inside the Den folder,
// saves only the Den's tiles, and on each save writes the terrain delta (<name>.terrain.json): heights,
// texture layers by file name, holes, water, vertex colours and model placements by file name and
// transform. Numbers and names only: the delta never holds a byte of a game file.

#include <forge/den_mode.hpp>

#include <noggit/ChunkWater.hpp>
#include <noggit/Log.h>
#include <noggit/MapChunk.h>
#include <noggit/MapHeaders.h>
#include <noggit/MapTile.h>
#include <noggit/ModelInstance.h>
#include <noggit/Selection.h>
#include <noggit/WMOInstance.h>
#include <noggit/World.h>
#include <noggit/liquid_layer.hpp>
#include <noggit/texture_set.hpp>
#include <noggit/tile_index.hpp>

#include <nlohmann/json.hpp>

#include <QtCore/QByteArray>
#include <QtCore/QDateTime>
#include <QtCore/QDir>
#include <QtCore/QFile>
#include <QtCore/QFileInfo>
#include <QtCore/QRegExp>
#include <QtCore/QSaveFile>
#include <QtCore/QSettings>
#include <QtWidgets/QMessageBox>

#include <glm/gtc/matrix_transform.hpp>

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <map>
#include <mutex>
#include <set>
#include <sstream>

namespace forge
{
  namespace den
  {
    namespace
    {
      options g_options;
      std::vector<prop_marker> g_props;
      std::mutex g_mutex;
      std::set<std::pair<std::size_t, std::size_t>> g_loaded_from_archives;  // waiting for a base snapshot
      std::vector<std::string> g_refused;

      constexpr std::size_t max_delta_bytes = 4u * 1024u * 1024u;
      constexpr std::size_t max_den_bytes = 512u * 1024u;
      constexpr char const* delta_format = "leyline-terrain";
      constexpr int delta_version = 1;

      QString clean_dir(QString const& path)
      {
        return QDir::cleanPath(QDir(path).absolutePath());
      }

      bool is_inside(QString const& path, QString const& root)
      {
        if (root.isEmpty())
        {
          return false;
        }
        QString const p = QDir::cleanPath(path);
        QString const r = QDir::cleanPath(root);
        return p.compare(r, Qt::CaseInsensitive) == 0
          || p.startsWith(r + "/", Qt::CaseInsensitive);
      }

      bool parse_pair(QString const& text, int* a, int* b)
      {
        QStringList parts = text.split(QRegExp("[,_ x]"), QString::SkipEmptyParts);
        if (parts.size() != 2)
        {
          return false;
        }
        bool ok_a = false, ok_b = false;
        *a = parts[0].trimmed().toInt(&ok_a);
        *b = parts[1].trimmed().toInt(&ok_b);
        return ok_a && ok_b && *a >= 0 && *a < 64 && *b >= 0 && *b < 64;
      }

      bool read_small_file(QString const& path, std::size_t cap, QByteArray* out, QString* error)
      {
        QFile f(path);
        if (!f.open(QIODevice::ReadOnly))
        {
          *error = "Can't open " + path;
          return false;
        }
        if (static_cast<std::size_t>(f.size()) > cap)
        {
          *error = path + " is too big";
          return false;
        }
        *out = f.readAll();
        return true;
      }

      bool read_locked_tiles_file(QString const& path, QString* error)
      {
        QByteArray bytes;
        if (!read_small_file(path, 64 * 1024, &bytes, error))
        {
          return false;
        }
        QString const text = QString::fromUtf8(bytes).trimmed();
        if (text.startsWith("["))
        {
          try
          {
            auto j = nlohmann::json::parse(bytes.constData(), bytes.constData() + bytes.size());
            for (auto const& e : j)
            {
              int x = -1, y = -1;
              if (e.is_array() && e.size() == 2 && e[0].is_number_integer() && e[1].is_number_integer())
              {
                x = e[0].get<int>();
                y = e[1].get<int>();
              }
              else if (e.is_string() && parse_pair(QString::fromStdString(e.get<std::string>()), &x, &y))
              {
              }
              if (x < 0 || x > 63 || y < 0 || y > 63)
              {
                *error = "Bad tile in " + path;
                return false;
              }
              g_options.allowed_tiles.emplace_back(x, y);
            }
          }
          catch (std::exception const& e)
          {
            *error = "Bad tile list " + path + ": " + e.what();
            return false;
          }
          return true;
        }
        for (QString line : text.split('\n'))
        {
          line = line.trimmed();
          if (line.isEmpty() || line.startsWith('#'))
          {
            continue;
          }
          int x, y;
          if (!parse_pair(line, &x, &y))
          {
            *error = "Bad line in " + path + ": " + line;
            return false;
          }
          g_options.allowed_tiles.emplace_back(x, y);
        }
        return true;
      }

      // The Den file: only its name, its origin (if given) and its props' positions are read.
      bool read_den_file(QString const& path, QString* error)
      {
        QByteArray bytes;
        if (!read_small_file(path, max_den_bytes, &bytes, error))
        {
          return false;
        }
        try
        {
          auto j = nlohmann::json::parse(bytes.constData(), bytes.constData() + bytes.size());
          if (!j.is_object() || j.value("format", std::string()) != "leyline-den")
          {
            *error = path + " is not a Den file";
            return false;
          }
          if (g_options.name.isEmpty() && j.contains("name") && j["name"].is_string())
          {
            g_options.name = QString::fromStdString(j["name"].get<std::string>());
          }
          auto num = [] (nlohmann::json const& o, char const* key, float def)
          {
            return (o.contains(key) && o[key].is_number()) ? o[key].get<float>() : def;
          };
          if (j.contains("props") && j["props"].is_array())
          {
            for (auto const& p : j["props"])
            {
              if (!p.is_object() || g_props.size() >= 800)
              {
                continue;
              }
              prop_marker m;
              m.world = glm::vec3(num(p, "x", 0.f), num(p, "y", 0.f), num(p, "z", 0.f));
              m.yaw = num(p, "yaw", 0.f);
              m.scale = std::clamp(num(p, "scale", 1.f), 0.1f, 5.f);
              g_props.push_back(m);
            }
          }
        }
        catch (std::exception const& e)
        {
          *error = "Bad Den file " + path + ": " + e.what();
          return false;
        }
        return true;
      }

      std::string iso_now()
      {
        return QDateTime::currentDateTimeUtc().toString(Qt::ISODate).toStdString();
      }

      std::string b64(void const* data, std::size_t size)
      {
        return QByteArray(static_cast<char const*>(data), static_cast<int>(size)).toBase64().toStdString();
      }

      std::uint64_t fnv1a(std::string const& s)
      {
        std::uint64_t h = 1469598103934665603ull;
        for (unsigned char c : s)
        {
          h ^= c;
          h *= 1099511628211ull;
        }
        return h;
      }

      unsigned char color_byte(float v)
      {
        return static_cast<unsigned char>(std::clamp(std::lround(v * 127.0f), 0L, 255L));
      }

      std::string hex64(std::uint64_t v)
      {
        std::ostringstream o;
        o << std::hex << v;
        return o.str();
      }

      QString forge_dir()
      {
        return g_options.project + "/.forge";
      }

      QString base_snapshot_path(int map, std::size_t x, std::size_t y)
      {
        return QString("%1/base/%2_%3_%4.json").arg(forge_dir()).arg(map).arg(x).arg(y);
      }

      QString delta_path(std::size_t x, std::size_t y)
      {
        bool const main_tile = static_cast<int>(x) == g_options.tile_x && static_cast<int>(y) == g_options.tile_y;
        if (main_tile)
        {
          return QString("%1/%2.terrain.json").arg(g_options.project, g_options.name);
        }
        return QString("%1/%2.%3_%4.terrain.json").arg(g_options.project, g_options.name).arg(x).arg(y);
      }

      // One chunk as the delta writes it. The entry is the chunk's whole state.
      // For a fingerprint, heights are rounded to millimetres: a save and reload can move a float by one bit.
      nlohmann::json chunk_json(MapChunk* chunk, int index, bool fingerprint = false)
      {
        nlohmann::json c;
        c["i"] = index;

        if (fingerprint)
        {
          nlohmann::json mm = nlohmann::json::array();
          for (int v = 0; v < mapbufsize; ++v)
          {
            mm.push_back(std::lround(chunk->mVertices[v].y * 1000.0));
          }
          c["heights"] = mm;
        }
        else
        {
          float heights[mapbufsize];
          for (int v = 0; v < mapbufsize; ++v)
          {
            heights[v] = chunk->mVertices[v].y;
          }
          c["heights"] = b64(heights, sizeof(heights));
        }
        c["normals"] = false;

        nlohmann::json layers = nlohmann::json::array();
        if (TextureSet* ts = chunk->getTextureSet())
        {
          std::vector<std::vector<uint8_t>> alphas = ts->save_alpha(true);
          for (std::size_t l = 0; l < ts->num(); ++l)
          {
            nlohmann::json layer;
            layer["tex"] = ts->filename(l);
            layer["flags"] = ts->flag(l);
            if (l > 0 && l - 1 < alphas.size() && alphas[l - 1].size() == 4096)
            {
              layer["alpha"] = b64(alphas[l - 1].data(), 4096);
            }
            layers.push_back(layer);
          }
        }
        c["layers"] = layers;

        c["holes"] = static_cast<unsigned>(chunk->holes) & 0xFFFFu;

        if (ChunkWater* water = chunk->liquid_chunk())
        {
          auto* list = water->getLayers();
          if (list && !list->empty() && !list->front().empty())
          {
            liquid_layer& l = list->front();
            std::uint64_t mask = l.getSubchunks();
            unsigned char bytes[8];
            for (int b = 0; b < 8; ++b)
            {
              bytes[b] = static_cast<unsigned char>((mask >> (8 * b)) & 0xFF);
            }
            if (fingerprint)
            {
              c["water"] = {{"type", l.liquidID()}, {"min", std::lround(l.min() * 1000.0)},
                            {"max", std::lround(l.max() * 1000.0)}, {"mask", b64(bytes, 8)}};
            }
            else
            {
              c["water"] = {{"type", l.liquidID()}, {"min", l.min()}, {"max", l.max()}, {"mask", b64(bytes, 8)}};
            }
          }
        }

        if (chunk->hasColors())
        {
          unsigned char colors[mapbufsize * 4];
          for (int v = 0; v < mapbufsize; ++v)
          {
            // The bytes as the map file stores them: blue, green, red, alpha (0x7F is 1.0).
            colors[v * 4 + 0] = color_byte(chunk->mccv[v].z);
            colors[v * 4 + 1] = color_byte(chunk->mccv[v].y);
            colors[v * 4 + 2] = color_byte(chunk->mccv[v].x);
            colors[v * 4 + 3] = 0;
          }
          c["colors"] = b64(colors, sizeof(colors));
        }

        c["area"] = chunk->areaID;
        c["impassable"] = static_cast<bool>(chunk->header_flags.flags.impass);
        return c;
      }

      // A model's fingerprint: positions to the millimetre, rotations to a thousandth of a degree.
      std::string model_fingerprint(nlohmann::json m)
      {
        for (char const* k : {"x", "y", "z", "rx", "ry", "rz", "scale"})
        {
          m[k] = std::lround(m[k].get<double>() * 1000.0);
        }
        return hex64(fnv1a(m.dump()));
      }

      struct placed
      {
        std::uint32_t uid;
        nlohmann::json json;
      };

      std::vector<placed> tile_models(World* world, MapTile* tile)
      {
        std::vector<placed> out;
        for (std::uint32_t uid : *tile->get_uids())
        {
          auto model = world->get_model(uid);
          if (!model || model.get().which() != eEntry_Object)
          {
            continue;
          }
          SceneObject* obj = boost::get<selected_object_type>(model.get());
          if (!obj)
          {
            continue;
          }
          glm::vec3 const p = obj->pos;
          float const wx = ZEROPOINT - p.z;
          float const wy = ZEROPOINT - p.x;
          float const wz = p.y;
          nlohmann::json m;
          m["path"] = obj->getFilename();
          m["uid"] = uid;
          m["x"] = wx - g_options.origin[0];
          m["y"] = wy - g_options.origin[1];
          m["z"] = wz - g_options.origin[2];
          m["rx"] = obj->dir.x;
          m["ry"] = obj->dir.y;
          m["rz"] = obj->dir.z;
          m["scale"] = obj->which() == eMODEL ? obj->scale : 1.0f;
          out.push_back({uid, m});
        }
        return out;
      }

      MapTile* loaded_tile(World* world, std::size_t x, std::size_t y)
      {
        tile_index const index(x, y);
        if (!world->mapIndex.tileLoaded(index))
        {
          return nullptr;
        }
        MapTile* tile = world->mapIndex.getTile(index);
        if (!tile || !tile->finishedLoading())
        {
          return nullptr;
        }
        return tile;
      }

      bool write_json_file(QString const& path, nlohmann::json const& j, std::size_t cap, QString* error)
      {
        std::string const text = j.dump(1);
        if (text.size() > cap)
        {
          *error = QString("%1 would be %2 MB, more than the %3 MB a delta may be; it was not written")
                     .arg(QFileInfo(path).fileName())
                     .arg(text.size() / (1024.0 * 1024.0), 0, 'f', 1)
                     .arg(cap / (1024 * 1024));
          return false;
        }
        if (!write_allowed(QFileInfo(path).absoluteFilePath().toStdString()))
        {
          *error = "Refused to write outside the Den folder: " + path;
          return false;
        }
        QDir().mkpath(QFileInfo(path).absolutePath());
        QSaveFile f(path);
        if (!f.open(QIODevice::WriteOnly))
        {
          *error = "Can't write " + path;
          return false;
        }
        f.write(text.data(), static_cast<qint64>(text.size()));
        if (!f.commit())
        {
          *error = "Can't write " + path;
          return false;
        }
        return true;
      }

      void make_base_snapshot(World* world, MapTile* tile, std::size_t x, std::size_t y)
      {
        nlohmann::json snap;
        snap["format"] = "forge-base";
        snap["version"] = 1;
        snap["map"] = world->getMapID();
        snap["tile"] = {x, y};
        nlohmann::json chunks = nlohmann::json::array();
        for (int i = 0; i < 256; ++i)
        {
          MapChunk* chunk = tile->getChunk(i % 16, i / 16);
          chunks.push_back(chunk ? hex64(fnv1a(chunk_json(chunk, i, true).dump())) : std::string());
        }
        snap["chunks"] = chunks;
        nlohmann::json models = nlohmann::json::object();
        for (auto const& m : tile_models(world, tile))
        {
          models[std::to_string(m.uid)] = model_fingerprint(m.json);
        }
        snap["models"] = models;
        QString error;
        if (!write_json_file(base_snapshot_path(world->getMapID(), x, y), snap, max_delta_bytes, &error))
        {
          LogError << "Forge: base snapshot: " << error.toStdString() << std::endl;
        }
        else
        {
          Log << "Forge: base snapshot made for tile " << x << "," << y << std::endl;
        }
      }

      bool load_json(QString const& path, std::size_t cap, nlohmann::json* out)
      {
        QByteArray bytes;
        QString error;
        if (!QFileInfo::exists(path) || !read_small_file(path, cap, &bytes, &error))
        {
          return false;
        }
        try
        {
          *out = nlohmann::json::parse(bytes.constData(), bytes.constData() + bytes.size());
          return out->is_object();
        }
        catch (...)
        {
          return false;
        }
      }

      // Writes one tile's delta. Returns a line for the save report.
      QString write_delta(World* world, std::size_t x, std::size_t y)
      {
        MapTile* tile = loaded_tile(world, x, y);
        if (!tile)
        {
          return QString("Tile %1,%2 is not loaded: its delta was not changed.").arg(x).arg(y);
        }
        int const map = world->getMapID();
        QString const path = delta_path(x, y);

        nlohmann::json previous;
        bool const has_previous = load_json(path, max_delta_bytes, &previous)
          && previous.value("format", std::string()) == delta_format;

        nlohmann::json base;
        bool const has_base = load_json(base_snapshot_path(map, x, y), max_delta_bytes, &base)
          && base.contains("chunks") && base["chunks"].is_array() && base["chunks"].size() == 256;

        std::set<int> open;
        bool has_open = false;
        if (has_previous && previous.contains("open") && previous["open"].is_array())
        {
          has_open = true;
          for (auto const& v : previous["open"])
          {
            if (v.is_number_integer() && v.get<int>() >= 0 && v.get<int>() < 256)
            {
              open.insert(v.get<int>());
            }
          }
        }

        nlohmann::json delta;
        delta["format"] = delta_format;
        delta["version"] = delta_version;
        delta["base"] = (has_previous && previous.contains("base") && previous["base"].is_string())
          ? previous["base"].get<std::string>()
          : QString("own:%1/%2/%3").arg(map).arg(x).arg(y).toStdString();

        nlohmann::json chunks = nlohmann::json::array();
        std::set<int> changed;
        int outside = 0;
        for (int i = 0; i < 256; ++i)
        {
          MapChunk* chunk = tile->getChunk(i % 16, i / 16);
          if (!chunk)
          {
            continue;
          }
          if (has_base && base["chunks"][i].is_string()
              && base["chunks"][i].get<std::string>() == hex64(fnv1a(chunk_json(chunk, i, true).dump())))
          {
            continue;  // the same as the base: not in the file
          }
          if (has_open && !open.count(i))
          {
            if (has_base)
            {
              ++outside;  // changed, but this chunk isn't open in the Den
            }
            continue;
          }
          changed.insert(i);
          chunks.push_back(chunk_json(chunk, i));
        }

        nlohmann::json open_json = nlohmann::json::array();
        for (int i : (has_open ? open : changed))
        {
          open_json.push_back(i);
        }
        delta["open"] = open_json;
        if (has_previous && previous.contains("pad") && previous["pad"].is_object())
        {
          delta["pad"] = previous["pad"];
        }
        delta["chunks"] = chunks;

        nlohmann::json models = nlohmann::json::array();
        int removed = 0;
        std::set<std::string> present;
        for (auto const& m : tile_models(world, tile))
        {
          std::string const key = std::to_string(m.uid);
          present.insert(key);
          if (has_base && base.contains("models") && base["models"].is_object()
              && base["models"].contains(key) && base["models"][key].is_string()
              && base["models"][key].get<std::string>() == model_fingerprint(m.json))
          {
            continue;  // a base model, unmoved
          }
          models.push_back(m.json);
        }
        if (has_base && base.contains("models") && base["models"].is_object())
        {
          for (auto const& item : base["models"].items())
          {
            if (!present.count(item.key()))
            {
              ++removed;
            }
          }
        }
        delta["models"] = models;
        delta["made"] = {{"by", "forge"}, {"at", iso_now()}};

        QString error;
        if (!write_json_file(path, delta, max_delta_bytes, &error))
        {
          return error;
        }
        QString line = QString("%1: %2 changed chunks, %3 models.")
                         .arg(QFileInfo(path).fileName()).arg(chunks.size()).arg(models.size());
        if (!has_base)
        {
          line += " (No base snapshot for this tile: every open chunk was written.)";
        }
        if (outside)
        {
          line += QString(" %1 changed chunks are outside the Den's open ground and were left out.").arg(outside);
        }
        if (removed)
        {
          line += QString(" %1 of the map's own models were removed; a delta can't remove them yet.").arg(removed);
        }
        return line;
      }
    }

    QString usage()
    {
      return
        "Forge, Den mode:\n"
        "  --game <folder>          the game folder (read only)\n"
        "  --project <folder>       the Den folder: Forge writes only here\n"
        "  --map <id>               the map to open\n"
        "  --tile <x,y>             the Den's tile; Forge opens there\n"
        "  --locked-tiles [<file>]  only the Den's tile (or the tiles listed in <file>) can be saved\n"
        "  --den <file.den.json>    show the Den's props as reference markers\n"
        "  --name <name>            the delta's name (<name>.terrain.json); default: the Den file's name\n"
        "  --origin <x,y,z>         the Den's middle in world yards\n";
    }

    bool parse_command_line(QStringList const& args, QString* error)
    {
      options o;
      bool any = false;
      bool want_locked_file = false;
      QString locked_file;

      for (int i = 1; i < args.size(); ++i)
      {
        QString const a = args[i];
        auto next = [&] (QString* out) -> bool
        {
          if (i + 1 >= args.size() || args[i + 1].startsWith("--"))
          {
            *error = a + " needs a value.\n\n" + usage();
            return false;
          }
          *out = args[++i];
          return true;
        };
        QString v;
        if (a == "--game")         { if (!next(&o.game)) return false; any = true; }
        else if (a == "--project") { if (!next(&o.project)) return false; any = true; }
        else if (a == "--den")     { if (!next(&o.den_file)) return false; any = true; }
        else if (a == "--name")    { if (!next(&o.name)) return false; }
        else if (a == "--map")
        {
          if (!next(&v)) return false;
          bool ok = false;
          o.map = v.toInt(&ok);
          if (!ok || o.map < 0) { *error = "Bad --map " + v; return false; }
          any = true;
        }
        else if (a == "--tile")
        {
          if (!next(&v)) return false;
          if (!parse_pair(v, &o.tile_x, &o.tile_y)) { *error = "Bad --tile " + v + " (use x,y, 0 to 63)"; return false; }
          any = true;
        }
        else if (a == "--locked-tiles")
        {
          o.locked = true;
          any = true;
          if (i + 1 < args.size() && !args[i + 1].startsWith("--"))
          {
            locked_file = args[++i];
            want_locked_file = true;
          }
        }
        else if (a == "--origin")
        {
          if (!next(&v)) return false;
          QStringList parts = v.split(',');
          bool ok = parts.size() == 3;
          for (int k = 0; ok && k < 3; ++k)
          {
            o.origin[k] = parts[k].trimmed().toFloat(&ok);
            ok = ok && std::isfinite(o.origin[k]);
          }
          if (!ok) { *error = "Bad --origin " + v + " (use x,y,z)"; return false; }
        }
        else if (a == "--help" || a == "-h")
        {
          *error = usage();
          return false;
        }
        else if (a.startsWith("--"))
        {
          *error = "Unknown option " + a + "\n\n" + usage();
          return false;
        }
      }

      if (!any)
      {
        g_options = options{};
        return true;
      }

      if (o.game.isEmpty() || o.project.isEmpty())
      {
        *error = "Den mode needs both --game and --project.\n\n" + usage();
        return false;
      }
      o.game = clean_dir(o.game);
      o.project = clean_dir(o.project);
      if (!QDir(o.game).exists())
      {
        *error = "The game folder doesn't exist: " + o.game;
        return false;
      }
      if (is_inside(o.project, o.game) || is_inside(o.game, o.project))
      {
        *error = "The Den folder must be outside the game folder (and the game folder outside the Den folder).";
        return false;
      }
      if (!QDir().mkpath(o.project))
      {
        *error = "Can't make the Den folder: " + o.project;
        return false;
      }
      if (o.map < 0 || o.tile_x < 0)
      {
        *error = "Den mode needs --map and --tile.\n\n" + usage();
        return false;
      }

      o.active = true;
      g_options = o;
      g_props.clear();
      g_options.allowed_tiles.clear();
      g_options.allowed_tiles.emplace_back(o.tile_x, o.tile_y);

      if (want_locked_file && !read_locked_tiles_file(locked_file, error))
      {
        g_options = options{};
        return false;
      }
      if (!g_options.den_file.isEmpty() && !read_den_file(g_options.den_file, error))
      {
        g_options = options{};
        return false;
      }
      if (g_options.name.isEmpty())
      {
        g_options.name = QDir(g_options.project).dirName();
      }
      // Only letters, digits, - and _ in the delta's name.
      g_options.name.replace(QRegExp("[^A-Za-z0-9_-]"), "_");
      if (g_options.name.isEmpty())
      {
        g_options.name = "den";
      }
      return true;
    }

    options const& get() { return g_options; }
    bool active() { return g_options.active; }

    void prepare_settings_scope()
    {
      if (!active())
      {
        return;
      }
      QDir().mkpath(forge_dir());
      QSettings::setDefaultFormat(QSettings::IniFormat);
      QSettings::setPath(QSettings::IniFormat, QSettings::UserScope, forge_dir());
      QSettings::setPath(QSettings::IniFormat, QSettings::SystemScope, forge_dir());
    }

    void apply_paths_to_settings()
    {
      if (!active())
      {
        return;
      }
      QSettings settings;
      settings.setValue("project/game_path", g_options.game);
      settings.setValue("project/path", g_options.project + "/");
      settings.setValue("uid_startup_check", false);
      settings.sync();
    }

    std::string log_path()
    {
      return active() ? (forge_dir() + "/log.txt").toStdString() : std::string("log.txt");
    }

    bool tile_allowed(std::size_t x, std::size_t z)
    {
      if (!active() || !g_options.locked)
      {
        return true;
      }
      return std::any_of(g_options.allowed_tiles.begin(), g_options.allowed_tiles.end(),
                         [&] (auto const& t) { return t.first == static_cast<int>(x) && t.second == static_cast<int>(z); });
    }

    bool write_allowed(std::string const& absolute_path)
    {
      if (!active())
      {
        return true;
      }
      QString const p = QDir::cleanPath(QFileInfo(QString::fromStdString(absolute_path)).absoluteFilePath());
      return is_inside(p, g_options.project) && !is_inside(p, g_options.game);
    }

    void note_refused(std::string const& what)
    {
      std::lock_guard<std::mutex> lock(g_mutex);
      LogError << "Forge: refused: " << what << std::endl;
      if (g_refused.size() < 50)
      {
        g_refused.push_back(what);
      }
    }

    void note_tile_loaded(std::size_t x, std::size_t z, bool from_archives)
    {
      if (!active() || !from_archives)
      {
        return;
      }
      bool wanted = std::any_of(g_options.allowed_tiles.begin(), g_options.allowed_tiles.end(),
                                [&] (auto const& t) { return t.first == static_cast<int>(x) && t.second == static_cast<int>(z); });
      if (wanted)
      {
        std::lock_guard<std::mutex> lock(g_mutex);
        g_loaded_from_archives.emplace(x, z);
      }
    }

    void tick(World* world)
    {
      if (!active() || !world)
      {
        return;
      }
      static std::set<std::pair<std::size_t, std::size_t>> uid_checked;
      for (auto const& t : g_options.allowed_tiles)
      {
        auto const key = std::make_pair(static_cast<std::size_t>(t.first), static_cast<std::size_t>(t.second));
        if (uid_checked.count(key))
        {
          continue;
        }
        if (MapTile* tile = loaded_tile(world, key.first, key.second))
        {
          // Models placed in an earlier session keep their ids: new ones start above them.
          for (std::uint32_t uid : *tile->get_uids())
          {
            if (uid >= den_uid_base)
            {
              world->mapIndex.ensureUIDAbove(uid);
            }
          }
          uid_checked.insert(key);
        }
      }
      std::vector<std::pair<std::size_t, std::size_t>> pending;
      {
        std::lock_guard<std::mutex> lock(g_mutex);
        pending.assign(g_loaded_from_archives.begin(), g_loaded_from_archives.end());
      }
      for (auto const& t : pending)
      {
        MapTile* tile = loaded_tile(world, t.first, t.second);
        if (!tile)
        {
          continue;
        }
        make_base_snapshot(world, tile, t.first, t.second);
        std::lock_guard<std::mutex> lock(g_mutex);
        g_loaded_from_archives.erase(t);
      }
    }

    void after_save(World* world, QWidget* parent)
    {
      if (!active() || !world)
      {
        return;
      }
      QStringList report;
      if (world->getMapID() == g_options.map)
      {
        for (auto const& t : g_options.allowed_tiles)
        {
          report << write_delta(world, t.first, t.second);
        }
      }
      std::vector<std::string> refused;
      {
        std::lock_guard<std::mutex> lock(g_mutex);
        refused.swap(g_refused);
      }
      for (auto const& line : report)
      {
        Log << "Forge: " << line.toStdString() << std::endl;
      }
      if (!refused.empty())
      {
        QString text = "Only your Den's ground can be saved. These were not saved:\n";
        for (auto const& r : refused)
        {
          text += "  " + QString::fromStdString(r) + "\n";
        }
        QMessageBox::warning(parent, "Forge", text + "\n" + report.join("\n"));
      }
    }

    std::vector<prop_marker> const& reference_props()
    {
      return g_props;
    }

    glm::vec3 to_editor_position(glm::vec3 const& world)
    {
      // Editor space: x = ZEROPOINT - world y, y = height, z = ZEROPOINT - world x.
      return glm::vec3(ZEROPOINT - world.y, world.z, ZEROPOINT - world.x);
    }

    glm::vec3 start_camera_position()
    {
      glm::vec3 const middle(g_options.origin[0], g_options.origin[1], g_options.origin[2]);
      bool const origin_on_tile =
        static_cast<int>(std::floor((ZEROPOINT - middle.y) / TILESIZE)) == g_options.tile_x
        && static_cast<int>(std::floor((ZEROPOINT - middle.x) / TILESIZE)) == g_options.tile_y;
      if (origin_on_tile)
      {
        glm::vec3 p = to_editor_position(middle);
        p.y += 50.f;
        return p;
      }
      // The middle of the Den's tile.
      return glm::vec3((g_options.tile_x + 0.5f) * TILESIZE, 120.f, (g_options.tile_y + 0.5f) * TILESIZE);
    }
  }
}
