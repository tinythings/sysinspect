#include "json.hpp"
#include <fstream>
#include <iostream>
#include <string>

using json = nlohmann::json;

class MemInfo {
  private:
    mutable std::ifstream meminfo_;
    long memavail_kb;
    long memtotal_kb;
    long memfree_kb;

    // Parse memory information from a specific line
    long parseMemKey(const std::string &line, const std::string &key) {
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
    void parseMemInfo(const std::string &filename) {
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

  public:
    MemInfo() { parseMemInfo("/proc/meminfo"); }
    ~MemInfo() {
        if (meminfo_.is_open()) {
            meminfo_.close();
        }
    }

    long getMemFree() const { return memfree_kb; }
    long getMemTotal() const { return memtotal_kb; }
    long getMemAvailable() const { return memavail_kb; }
};

int main() {
    MemInfo memInfo;
    long memfree_kb = memInfo.getMemFree();
    if (memfree_kb == -1) {
        return 1;
    }

    // JSON output
    json jout;
    jout["mem-free"] = memfree_kb;
    jout["mem-total"] = memInfo.getMemTotal();
    jout["mem-available"] = memInfo.getMemAvailable();
    std::cout << jout.dump(2) << std::endl;

    return 0;
}
