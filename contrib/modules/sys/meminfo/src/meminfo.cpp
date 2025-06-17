#include "meminfo.hpp"
#include "nlohmann/json.hpp"
#include <charconv>
#include <fstream>
#include <iostream>
#include <string>

using json = nlohmann::json;

MemInfo::MemInfo() : memavail_kb(-1), memtotal_kb(-1), memfree_kb(-1) { parseMemInfo("/proc/meminfo"); }
MemInfo::~MemInfo() {
    if (meminfo_.is_open()) {
        meminfo_.close();
    }
}

// Parse memory information from a specific line
long MemInfo::parseMemKey(const std::string &line, const std::string &key) {
    size_t offset = line.find(key);
    if (offset != std::string::npos) {
        size_t start = line.find_first_of("0123456789", offset + key.length());
        if (start != std::string::npos) {
            std::string nstr = line.substr(start, line.find_first_not_of("0123456789", start) - start);
            long value = 0;
            auto [ptr, ec] = std::from_chars(nstr.data(), nstr.data() + nstr.size(), value);
            if (ec == std::errc()) {
                return value;
            } else {
                return -1;
            }
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
