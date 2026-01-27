// main.go
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"
)

func main() {
	b, err := os.ReadFile("/etc/machine-id")
	if err != nil {
		out, _ := json.Marshal(map[string]string{
			"error": "failed to read /etc/machine-id",
		})
		fmt.Println(string(out))
		return
	}

	id := strings.TrimSpace(string(b))
	out, _ := json.Marshal(map[string]string{
		"machine_id": id,
	})
	fmt.Println(string(out))
}
