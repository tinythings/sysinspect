package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"strings"
)

type Header struct {
	Opts []string               `json:"opts"`
	Args map[string]interface{} `json:"args"`
}

func scalar2bool(v interface{}) bool {
	switch t := v.(type) {
	case bool:
		return t
	case string:
		s := strings.ToLower(strings.TrimSpace(t))
		return s != "" && s != "0" && s != "false" && s != "no"
	case float64:
		return t != 0
	default:
		return false
	}
}

func readHeader() (Header, error) {
	in := bufio.NewScanner(os.Stdin)
	in.Buffer(make([]byte, 0, 64*1024), 10*1024*1024)

	var hdr Header
	if !in.Scan() {
		if err := in.Err(); err != nil {
			return hdr, fmt.Errorf("stdin scan: %w", err)
		}
		return hdr, fmt.Errorf("missing header JSON on stdin")
	}
	if err := json.Unmarshal(in.Bytes(), &hdr); err != nil {
		return hdr, fmt.Errorf("failed to parse header JSON: %w", err)
	}
	return hdr, nil
}

// Small parser for /etc/os-release file
func readOSRelease() (map[string]string, error) {
	b, err := os.ReadFile("/etc/os-release")
	if err != nil {
		return nil, err
	}
	out := map[string]string{}
	for _, line := range strings.Split(string(b), "\n") {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		k, v, ok := strings.Cut(line, "=")
		if !ok {
			continue
		}
		k = strings.TrimSpace(k)
		v = strings.TrimSpace(v)
		v = strings.Trim(v, `"'`)
		out[k] = v
	}
	return out, nil
}

// Module documentation
func doc() map[string]any {
	// SAME SHAPE as your Lua docs: arguments/options/examples arrays, returns object.
	return map[string]any{
		"name":        "hellodude",
		"version":     "0.1.0",
		"author":      "Gru",
		"description": "Says hello and returns OS version from /etc/os-release.",

		"arguments": []any{
			// none required; keep it empty array, not null, not map.
		},

		"options": []any{
			// none
		},

		"examples": []any{
			map[string]any{
				"description": "Get module output",
				"code":        `{ "args": { "mod": "hellodude" } }`,
			},
			map[string]any{
				"description": "Get module documentation",
				"code":        `{ "args": { "mod": "hellodude", "rt.man": true } }`,
			},
		},

		"returns": map[string]any{
			"description": "Returns a greeting and OS release info (if accessible).",
			"sample": map[string]any{
				"output": "hello, dude",
				"os": map[string]any{
					"NAME":        "Debian GNU/Linux",
					"VERSION_ID":  "12",
					"PRETTY_NAME": "Debian GNU/Linux 12 (bookworm)",
				},
			},
		},
	}
}

// Run the module logic
func run(hdr Header) map[string]any {
	_ = hdr // args/opts currently unused, but kept for future “CfgMgmt shit”.

	osr, err := readOSRelease()
	if err != nil {
		return map[string]any{
			"error":  "failed to read /etc/os-release",
			"detail": err.Error(),
		}
	}

	// Return the module data
	return map[string]any{
		"output":  "Hello, world!",
		"VERSION": osr["VERSION"],
	}
}

// WASI entry function
func main() {
	hdr, err := readHeader()
	if err != nil {
		fmt.Fprintln(os.Stderr, err.Error())
		os.Exit(1)
	}

	// doc-mode: args["rt.man"] == true
	if hdr.Args != nil && scalar2bool(hdr.Args["rt.man"]) {
		b, _ := json.Marshal(doc())
		fmt.Println(string(b))
		return
	}

	b, _ := json.Marshal(run(hdr))
	fmt.Println(string(b))
}
