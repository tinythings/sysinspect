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

func hasOpt(opts []string, want string) bool {
	for _, o := range opts {
		if o == want {
			return true
		}
	}
	return false
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

// Documentation of the module
//
// This is an example how to document your module so that sysinspect can
// generate manual pages and help texts automatically.
//
// Important is to keep the structure and the field names as they are here.
func doc() map[string]any {
	return map[string]any{
		"name":        "hellodude",
		"version":     "0.1.0",
		"author":      "Gru",
		"description": "Says hello and returns OS version from /etc/os-release.",

		"arguments": []any{
			map[string]any{
				"name":        "key",
				"type":        "string",
				"description": "A key inside the /etc/os-release file to retrieve (not used in this example). Default: VERSION",
				"required":    true,
			},
		},
		"options": []any{
			map[string]any{
				"name":        "nohello",
				"description": "Do not say hello",
			},
		},

		"examples": []any{
			map[string]any{
				"description": "Get module output",
				"code":        `{ "args": { "key": "VERSION" } }`,
			},
			map[string]any{
				"description": "Get module documentation",
				"code":        `{ "args": { "key": "VERSION" }, "opts": ["man"] }`,
			},
		},

		"returns": map[string]any{
			"description": "Returns greeting and OS release info (if accessible).",
			"sample": map[string]any{
				"output": "hello, dude",
				"os": map[string]any{
					"PRETTY_NAME": "Debian GNU/Linux 12 (bookworm)",
					"VERSION_ID":  "12",
				},
			},
		},
	}
}

func run(hdr Header) map[string]any {
	osr, err := readOSRelease()
	if err != nil {
		return map[string]any{
			"error":  "failed to read /etc/os-release",
			"detail": err.Error(),
		}
	}

	key := "VERSION" // Default key
	if hdr.Args != nil {
		if v, ok := hdr.Args["key"]; ok {
			if s, ok := v.(string); ok && strings.TrimSpace(s) != "" {
				key = strings.TrimSpace(s)
			}
		}
	}

	val, ok := osr[key]
	if !ok {
		return map[string]any{
			"error": "unknown os-release key",
			"key":   key,
		}
	}

	out := map[string]any{
		key: val,
	}
	if !hasOpt(hdr.Opts, "nohello") {
		out["output"] = "hello, dude"
	}

	return out
}

func main() {
	hdr, err := readHeader()
	if err != nil {
		fmt.Fprintln(os.Stderr, err.Error())
		os.Exit(1)
	}

	enc := json.NewEncoder(os.Stdout)
	enc.SetEscapeHTML(false)

	if hasOpt(hdr.Opts, "man") {
		_ = enc.Encode(doc())
	} else {
		_ = enc.Encode(run(hdr))
	}
}
