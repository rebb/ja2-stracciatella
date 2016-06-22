SET(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
SET(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
SET(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)

SET(CMAKE_OSX_ARCHITECTURES "i386" CACHE STRING "" FORCE)
SET(LOCAL_SDL_LIB "_build/lib-SDL-devel-1.2.15-macos-i386" CACHE STRING "" FORCE)
SET(LOCAL_BOOST_LIB ON CACHE BOOL "" FORCE)

SET(CMAKE_EXE_LINKER_FLAGS "-framework IOKit")
SET(CMAKE_EXE_LINKER_FLAGS "${CMAKE_EXE_LINKER_FLAGS} -framework Carbon")
SET(CMAKE_EXE_LINKER_FLAGS "${CMAKE_EXE_LINKER_FLAGS} -framework AudioToolbox")
SET(CMAKE_EXE_LINKER_FLAGS "${CMAKE_EXE_LINKER_FLAGS} -framework OpenGL")
