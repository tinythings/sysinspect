#include "meminfo.hpp"
#include "nlohmann/json.hpp"
#include <iostream>
using json = nlohmann::json;

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
