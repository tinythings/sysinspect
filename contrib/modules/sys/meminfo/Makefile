.DEFAULT_GOAL := build

setup:
	sudo apt install g++-12 libstdc++-12-dev libc++-dev build-essential clang clangd gdb

clean:
	echo "Cleaning up"
	rm -f meminfo

build:
	$(MAKE) clean
	clang++ -v -std=c++17 meminfo.cpp -o meminfo
