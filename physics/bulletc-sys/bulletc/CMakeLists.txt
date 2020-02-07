cmake_minimum_required(VERSION 3.0)
project(bulletc)

set(CMAKE_CXX_STANDARD 11)
set(CMAKE_CXX_FLAGS_DEBUG  "-g")

set(SRC example.cpp bulletc.hpp debug.cpp debug.hpp entity_collider.cpp dynworld.cpp common.hpp dynworld.hpp)

add_library(bulletc SHARED ${SRC})

if(CMAKE_BUILD_TYPE STREQUAL "Debug")
  target_compile_definitions(bulletc PUBLIC DEBUG)
endif()

find_package(Bullet REQUIRED)
target_include_directories(bulletc PRIVATE ${BULLET_INCLUDE_DIR})
target_link_libraries(bulletc ${BULLET_LIBRARIES})

install(TARGETS bulletc DESTINATION .)
