cmake_minimum_required(VERSION 3.0)
project(bulletc)

set(SRC bulletc.cpp bulletc.hpp debug.cpp debug.hpp)

add_library(bulletc SHARED ${SRC})

find_package(Bullet REQUIRED)
target_include_directories(bulletc PRIVATE ${BULLET_INCLUDE_DIR})
target_link_libraries(bulletc ${BULLET_LIBRARIES})

install(TARGETS bulletc DESTINATION .)
