# This file is part of Noggit3, licensed under GNU General Public License (version 3).
# Modified for Forge (see CHANGES-FROM-NOGGIT-RED.md): Forge.exe, its licences and notices; no game data.

install(TARGETS noggit DESTINATION .)
install(FILES LICENSE THIRD-PARTY-NOTICES.md CHANGES-FROM-NOGGIT-RED.md README.md DESTINATION .)
install(DIRECTORY scripts DESTINATION . FILES_MATCHING PATTERN "*.lua")

set(CPACK_GENERATOR "ZIP")
SET(CPACK_PACKAGE_NAME "Forge")
SET(CPACK_PACKAGE_FILE_NAME "Forge")
SET(CPACK_PACKAGE_VERSION "${FORGE_VERSION}")

include(CPack)
