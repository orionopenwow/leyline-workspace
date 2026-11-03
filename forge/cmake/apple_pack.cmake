# This file is part of Noggit3, licensed under GNU General Public License (version 3).
# Modified for Forge (see CHANGES-FROM-NOGGIT-RED.md): bundle name and id.

# Also link against libz for static's sake.
SET(FREETYPE_LIBRARIES ${FREETYPE_LIBRARIES} z)
set_target_properties ( noggit
                        PROPERTIES
                        MACOSX_BUNDLE_ICON_FILE
                        "noggit"
                      )
set_source_files_properties (media/noggit.icns PROPERTIES MACOSX_PACKAGE_LOCATION "Resources")
set_target_properties ( noggit
                        PROPERTIES
                        MACOSX_BUNDLE_BUNDLE_NAME
                        "Forge"
                      )
set_target_properties ( noggit
                        PROPERTIES
                        MACOSX_BUNDLE_BUNDLE_VERSION
                        "3"
                      )
set_target_properties ( noggit
                        PROPERTIES
                        MACOSX_BUNDLE_GUI_IDENTIFIER
                        "org.leyline.forge"
                      )

# Copy the binary to bin/.
INSTALL(TARGETS noggit
  BUNDLE DESTINATION ${CMAKE_CURRENT_BINARY_DIR} COMPONENT Runtime
  RUNTIME DESTINATION bin COMPONENT Runtime
)

INCLUDE(InstallRequiredSystemLibraries)

SET(APPS "${CMAKE_CURRENT_BINARY_DIR}/noggit.app")

SET(DIRS ${OPENGL_LIBRARY_DIR} ${FREETYPE_LIBRARY_DIR} ${STORMLIB_LIBRARY_DIR} ${Boost_LIBRARY_DIR} ${GLEW_LIBRARY_DIR})
INSTALL(CODE " include(BundleUtilities) fixup_bundle(\"${APPS}\"   \"\"   \"${DIRS}\" " COMPONENT Runtime)

SET(CPACK_BUNDLE_NAME "Forge")
SET(CPACK_BUNDLE_PLIST "${CMAKE_SOURCE_DIR}/media/Info.plist")
SET(CPACK_BUNDLE_ICON "${CMAKE_SOURCE_DIR}/media/noggit.icns")
SET(CPACK_DMG_VOLUME_NAME "Forge")
SET(CPACK_DMG_BACKGROUND_IMAGE "${CMAKE_SOURCE_DIR}/media/noggit.png")
SET(CPACK_GENERATOR "DragNDrop")
SET(CPACK_PACKAGE_FILE_NAME "Forge")
INCLUDE(CPack)
