vcpkg_from_github(
    OUT_SOURCE_PATH SOURCE_PATH
    REPO ArtifexSoftware/mupdf
    REF 75a6b32b6676bd0676ea2f0c3be7deffa48f301b
    SHA512 ca5f502d7a48a04e73de930022f442d5b90127f244f983a44614c7f95c53f1591919c8811b82e802bb85460d35f9ae6dc1b57ebe7950f9808609c56735bcdb44
    HEAD_REF master
    PATCHES
)

vcpkg_find_acquire_program(GIT)
vcpkg_execute_required_process(
    COMMAND ${GIT} clone --recurse-submodules https://github.com/ArtifexSoftware/mupdf.git mupdf-full
    WORKING_DIRECTORY ${CURRENT_BUILDTREES_DIR}
    LOGNAME git-clone-submodules
)

vcpkg_execute_required_process(
    COMMAND ${GIT} checkout 75a6b32b6676bd0676ea2f0c3be7deffa48f301b
    WORKING_DIRECTORY ${CURRENT_BUILDTREES_DIR}/mupdf-full
    LOGNAME git-checkout
)

file(REMOVE_RECURSE ${SOURCE_PATH})
file(RENAME ${CURRENT_BUILDTREES_DIR}/mupdf-full ${SOURCE_PATH})

if(VCPKG_TARGET_IS_WINDOWS)
    vcpkg_build_msbuild(
        PROJECT_PATH ${SOURCE_PATH}/platform/win32/mupdf.sln
        TARGET libmupdf libthirdparty
        PLATFORM ${VCPKG_TARGET_ARCHITECTURE}
    )
    
    file(INSTALL ${SOURCE_PATH}/include/ DESTINATION ${CURRENT_PACKAGES_DIR}/include)
    
    if(VCPKG_TARGET_ARCHITECTURE STREQUAL "x64")
        set(ARCH_DIR x64)
    else()
        set(ARCH_DIR Win32)
    endif()
    
    file(INSTALL ${SOURCE_PATH}/platform/win32/${ARCH_DIR}/Release/libmupdf.lib 
         DESTINATION ${CURRENT_PACKAGES_DIR}/lib)
    file(INSTALL ${SOURCE_PATH}/platform/win32/${ARCH_DIR}/Release/libthirdparty.lib 
         DESTINATION ${CURRENT_PACKAGES_DIR}/lib)
         
    if(NOT VCPKG_BUILD_TYPE STREQUAL "release")
        file(INSTALL ${SOURCE_PATH}/platform/win32/${ARCH_DIR}/Debug/libmupdf.lib 
             DESTINATION ${CURRENT_PACKAGES_DIR}/debug/lib)
        file(INSTALL ${SOURCE_PATH}/platform/win32/${ARCH_DIR}/Debug/libthirdparty.lib 
             DESTINATION ${CURRENT_PACKAGES_DIR}/debug/lib)
    endif()
else()
    vcpkg_build_make(
        SOURCE_PATH ${SOURCE_PATH}
        BUILD_TARGET libs
        OPTIONS
            USE_SYSTEM_LIBS=yes
            HAVE_X11=no
            HAVE_GLUT=no
    )
    
    vcpkg_install_make(
        SOURCE_PATH ${SOURCE_PATH}
        BUILD_TARGET install-libs
        OPTIONS
            USE_SYSTEM_LIBS=yes
            HAVE_X11=no
            HAVE_GLUT=no
            prefix=${CURRENT_PACKAGES_DIR}
    )
endif()

file(INSTALL ${SOURCE_PATH}/COPYING DESTINATION ${CURRENT_PACKAGES_DIR}/share/${PORT} RENAME copyright)