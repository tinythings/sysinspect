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
CXXFLAGS := -std=c++17 -Os -flto -fdata-sections -ffunction-sections
LDFLAGS := -static -flto -Wl,--gc-sections -Wl,--strip-all -s

build: $(OBJS)
	@echo "Building meminfo for $(PLATFORM)"
	mkdir -p $(BINDIR)
	clang++ -v $(CXXFLAGS) $(OBJS) -o $(BINDIR)/meminfo

static-$(PLATFORM): $(OBJS)
	@echo "Building static meminfo for $(PLATFORM)"
	mkdir -p $(BINDIR)
	clang++ $(CXXFLAGS) $(LDFLAGS) $(OBJS) -o $(BINDIR)/meminfo.static
	@echo "Static build complete. You can find the binary at $(BINDIR)/meminfo.static"

$(OBJDIR)/%.o: src/%.cpp
	@echo "Building $@"
	@mkdir -p $(OBJDIR)
	clang++ -v $(CXXFLAGS) -c $< -o $@ $(INCLUDE)

.PHONY: setup clean build
