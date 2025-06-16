#pragma once

#include <fstream>
#include <string>

class MemInfo {
  public:
    MemInfo();
    ~MemInfo();
    long getMemFree() const;
    long getMemTotal() const;
    long getMemAvailable() const;

  private:
    mutable std::ifstream meminfo_;
    long memavail_kb;
    long memtotal_kb;
    long memfree_kb;
    long parseMemKey(const std::string &line, const std::string &key);
    void parseMemInfo(const std::string &filename);
};
