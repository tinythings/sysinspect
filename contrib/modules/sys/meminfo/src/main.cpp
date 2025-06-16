#include "meminfo.hpp"
#include "nlohmann/json.hpp"
#include <iostream>
#include <sstream>
using json = nlohmann::json;

int main() {
    // Get the JSON input from stdin
    std::stringstream buff;
    buff << std::cin.rdbuf();
    std::string input = buff.str();

    // Parse the input JSON
    json jin;
    try {
        jin = json::parse(input);
    } catch (const json::parse_error &e) {
        std::cerr << "JSON parse error: " << e.what() << std::endl;
        return 1;
    }

    if (!(jin.contains("opts") || jin.contains("options"))) {
        std::cerr << "Error: 'options' not specified!" << std::endl;
        return 1;
    }

    // What unit?
    std::string unit = "kb";
    if (jin.contains("args")) {
        if (jin["args"].contains("unit")) {
            unit = jin["args"]["unit"].get<std::string>();
            if (!(unit == "bt" || unit == "kb" || unit == "mb" || unit == "gb")) {
                unit = "kb";
            }
            std::transform(unit.begin(), unit.end(), unit.begin(), ::tolower);
        }
    }

    auto u = [&](long v) -> double {
        if (unit == "bt") {
            return static_cast<double>(v) * 1024.0;
        } else if (unit == "kb") {
            return static_cast<double>(v);
        } else if (unit == "mb") {
            return static_cast<double>(v) / 1024.0;
        } else if (unit == "gb") {
            return static_cast<double>(v) / (1024.0 * 1024.0);
        }
        return static_cast<double>(v);
    };

    MemInfo memInfo;

    // JSON output
    json jout;
    jout["retcode"] = 0;
    jout["message"] = "Data has been collected successfully";

    jout["data"] = json::object();
    jout["data"]["changed"] = true;
    jout["data"]["unit"] = unit;

    for (const auto &opt : jin.contains("opts") ? jin["opts"] : jin["options"]) {
        static const std::map<std::string, std::function<void()>> f = {
            {"free", [&]() { jout["data"]["mem-free"] = u(memInfo.getMemFree()); }},
            {"total", [&]() { jout["data"]["mem-total"] = u(memInfo.getMemTotal()); }},
            {"avail", [&]() { jout["data"]["mem-available"] = u(memInfo.getMemAvailable()); }}};

        auto it = f.find(opt.get<std::string>());
        if (it != f.end()) {
            it->second();
        } else {
            jout["retcode"] = 1;
            jout["message"] = "Unknown option: " + opt.get<std::string>();
            jout.erase("data");
            jout["data"]["changed"] = false;
            break;
        }
    }

    std::cout << jout.dump(2) << std::endl;

    return 0;
}
