#include "meminfo.hpp"
#include "nlohmann/json.hpp"
#include <fstream>
#include <iostream>
#include <string>

using json = nlohmann::json;

MemInfo::MemInfo() : memavail_kb(0), memtotal_kb(0), memfree_kb(0) { parseMemInfo("/proc/meminfo"); }
MemInfo::~MemInfo() {
    if (meminfo_.is_open()) {
        meminfo_.close();
    }
}

// Parse memory information from a specific line
long MemInfo::parseMemKey(const std::string &line, const std::string &key) {
    size_t pos = line.find(key);
    if (pos != std::string::npos) {
        size_t start = line.find_first_of("0123456789", pos + key.length());
        if (start != std::string::npos) {
            size_t end = line.find_first_not_of("0123456789", start);
            return std::stol(line.substr(start, end - start));
        }
    }
    return -1;
}

// Parse /proc/meminfo file to extract memory information
void MemInfo::parseMemInfo(const std::string &filename) {
    meminfo_.open(filename);
    if (!meminfo_.is_open()) {
        std::cerr << "Error: Could not open " << filename << std::endl;
        return;
    }
    std::string line;
    while (std::getline(meminfo_, line)) {
        if (line.find("MemAvailable:") != std::string::npos) {
            memavail_kb = parseMemKey(line, "MemAvailable");
        } else if (line.find("MemTotal:") != std::string::npos) {
            memtotal_kb = parseMemKey(line, "MemTotal");
        } else if (line.find("MemFree:") != std::string::npos) {
            memfree_kb = parseMemKey(line, "MemFree");
        }
    }
}

long MemInfo::getMemFree() const { return memfree_kb; }
long MemInfo::getMemTotal() const { return memtotal_kb; }
long MemInfo::getMemAvailable() const { return memavail_kb; }
