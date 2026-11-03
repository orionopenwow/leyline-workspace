// This file is part of Forge, based on Noggit Red; licensed under GNU General Public License (version 3).
// Forge change (not in Noggit Red): Den mode. See docs/DEN-MODE.md and CHANGES-FROM-NOGGIT-RED.md.

#pragma once

#include <glm/mat4x4.hpp>
#include <glm/vec3.hpp>
#include <glm/vec4.hpp>

#include <QtCore/QString>
#include <QtCore/QStringList>

#include <cstddef>
#include <string>
#include <utility>
#include <vector>

class World;
class MapTile;
class QWidget;

namespace forge
{
  namespace den
  {
    // Where the Den's middle is, in world yards, when neither --origin nor the Den file says otherwise.
    inline constexpr float default_origin_x = 16226.2f;
    inline constexpr float default_origin_y = 16257.0f;
    inline constexpr float default_origin_z = 13.2f;

    // New ground models placed in Den mode get unique ids from here up, so they never meet the base map's ids.
    inline constexpr unsigned den_uid_base = 0x7F000000u;

    struct prop_marker
    {
      glm::vec3 world;   // world yards (x, y, z up)
      float yaw = 0.f;   // degrees
      float scale = 1.f;
    };

    struct options
    {
      bool active = false;
      QString game;         // the player's game folder: read only
      QString project;      // the Den folder: the only place Forge writes to
      QString den_file;     // optional <name>.den.json, shown as reference markers
      QString name;         // the Den's name, used for <name>.terrain.json
      int map = -1;
      int tile_x = -1;
      int tile_y = -1;
      bool locked = false;  // only the allowed tiles can be saved
      std::vector<std::pair<int, int>> allowed_tiles;
      float origin[3] = {default_origin_x, default_origin_y, default_origin_z};
    };

    // Reads Forge's command line. Returns false with a message on a bad command line.
    // No Den flags at all leaves Den mode off (the editor behaves as before).
    bool parse_command_line(QStringList const& args, QString* error);
    QString usage();

    options const& get();
    bool active();

    // Settings: in Den mode every setting lives in <Den folder>/.forge/, never in the user's registry.
    void prepare_settings_scope();
    // Writes the Den's game and project paths into the (Den-scoped) settings.
    void apply_paths_to_settings();
    // Logging goes to <Den folder>/.forge/log.txt in Den mode.
    std::string log_path();

    // Saving rules.
    bool tile_allowed(std::size_t x, std::size_t z);
    bool write_allowed(std::string const& absolute_path);
    void note_refused(std::string const& what);

    // Hooks called by the editor.
    void note_tile_loaded(std::size_t x, std::size_t z, bool from_archives);
    void tick(World* world);
    void after_save(World* world, QWidget* parent);

    // Reference markers for the Den file's props, drawn by World::draw.
    std::vector<prop_marker> const& reference_props();
    glm::vec3 to_editor_position(glm::vec3 const& world);
    glm::vec3 start_camera_position();
  }
}
