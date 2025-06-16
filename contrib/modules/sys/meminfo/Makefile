.DEFAULT_GOAL := build
PLATFORM := $(shell uname -sm | tr ' ' '-' | tr '[:upper:]' '[:lower:]')
BINDIR := bin/$(PLATFORM)

setup:
	@sudo apt install g++-12 libstdc++-12-dev libc++-dev build-essential \
	clang clangd gdb musl-tools g++-aarch64-linux-gnu gcc-aarch64-linux-gnu \
	libyaml-cpp-dev

	# Prepare YAML cpp for ARM64
	@rm -rf tmp
	@mkdir -p tmp
	@cd tmp && git clone https://github.com/jbeder/yaml-cpp.git
	@cd tmp/yaml-cpp && mkdir build-arm64 && cd build-arm64 && \
	cmake -DCMAKE_SYSTEM_NAME=Linux -DCMAKE_SYSTEM_PROCESSOR=aarch64 \
      -DCMAKE_C_COMPILER=aarch64-linux-gnu-gcc \
      -DCMAKE_CXX_COMPILER=aarch64-linux-gnu-g++ \
      -DBUILD_SHARED_LIBS=OFF -DCMAKE_BUILD_TYPE=Release .. && \
	make
clean:
	@echo "Cleaning up"
	@rm -rf bin build

SRC := $(wildcard src/*.cpp)
OBJDIR := build
OBJS := $(patsubst src/%.cpp, $(OBJDIR)/%.o, $(SRC)) -lyaml-cpp
INCLUDE := -Iinclude
CXXFLAGS := -std=c++17 -O3 -flto -fdata-sections -ffunction-sections
LDFLAGS := -static -flto -Wl,--gc-sections -Wl,--strip-all -s

build: $(OBJS)
	@echo "Building meminfo for $(PLATFORM)"
	mkdir -p $(BINDIR)
	clang++ -v $(CXXFLAGS) $(OBJS) -o $(BINDIR)/meminfo

static-x86_64: $(OBJS)
	@echo "Building static meminfo for $(PLATFORM)"
	mkdir -p $(BINDIR)
	clang++ -v $(CXXFLAGS) $(LDFLAGS) $(OBJS) -o $(BINDIR)/meminfo.static
	@echo "Static build complete. You can find the binary at $(BINDIR)/meminfo.static"

static-arm64:
	mkdir -p bin/linux-arm64
	clang++ -v --target=aarch64-linux-gnu $(CXXFLAGS) $(LDFLAGS) $(INCLUDE) $(SRC) -Ltmp/yaml-cpp/build-arm64 -lyaml-cpp -o bin/linux-arm64/meminfo.static

$(OBJDIR)/%.o: src/%.cpp
	@echo "Building $@"
	@mkdir -p $(OBJDIR)
	@xxd -i mod_doc.yaml > include/mod_doc.hpp
	@echo "Refreshing mod_doc.h"
	clang++ -v $(CXXFLAGS) -c $< -o $@ $(INCLUDE)

.PHONY: setup clean build
