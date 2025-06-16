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

    MemInfo memInfo;

    // JSON output
    json jout;
    jout["retcode"] = 0;
    jout["message"] = "Data has been collected successfully";

    for (const auto &opt : jin.contains("opts") ? jin["opts"] : jin["options"]) {
        static const std::map<std::string, std::function<void()>> f = {
            {"free", [&]() { jout["data"]["mem-free"] = memInfo.getMemFree(); }},
            {"total", [&]() { jout["data"]["mem-total"] = memInfo.getMemTotal(); }},
            {"avail", [&]() { jout["data"]["mem-available"] = memInfo.getMemAvailable(); }}};

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
