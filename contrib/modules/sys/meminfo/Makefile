.DEFAULT_GOAL := build
PLATFORM := $(shell uname -sm | tr ' ' '-' | tr '[:upper:]' '[:lower:]')
BINDIR := bin/$(PLATFORM)

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
	@echo "Building meminfo for $(PLATFORM)"
	mkdir -p $(BINDIR)
	clang++ -v -std=c++17 $(OBJS) -o $(BINDIR)/meminfo

$(OBJDIR)/%.o: src/%.cpp
	@echo "Building $@"
	@mkdir -p $(OBJDIR)
	clang++ -v -std=c++17 -c $< -o $@ $(INCLUDE)

.PHONY: setup clean build
