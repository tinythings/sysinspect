.DEFAULT_GOAL := build

setup:
	sudo apt install g++-12 libstdc++-12-dev libc++-dev build-essential clang clangd gdb

clean:
	@echo "Cleaning up"
	@rm -rf bin build

SRC := $(wildcard src/*.cpp)
OBJDIR := build
OBJS := $(patsubst src/%.cpp, $(OBJDIR)/%.o, $(SRC))
INCLUDE := -Iinclude

build: $(OBJS)
	@echo "Building meminfo"
	mkdir -p bin
	clang++ -v -std=c++17 $(OBJS) -o bin/meminfo

$(OBJDIR)/%.o: src/%.cpp
	@echo "Building $@"
	@mkdir -p $(OBJDIR)
	clang++ -v -std=c++17 -c $< -o $@ $(INCLUDE)

.PHONY: setup clean build
