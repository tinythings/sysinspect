.DEFAULT_GOAL := build

setup:
	sudo apt install g++-12 libstdc++-12-dev libc++-dev build-essential clang clangd gdb

clean:
	echo "Cleaning up"
	rm -f meminfo src/*.o

SRC := $(wildcard src/*.cpp)
OBJS := $(SRC:.cpp=.o)
INCLUDE := -Iinclude

build: $(OBJS)
	clang++ -v -std=c++17 $(OBJS) -o meminfo

%.o: %.cpp
	clang++ -v -std=c++17 -c $< -o $@ $(INCLUDE)

.PHONY: setup clean build
