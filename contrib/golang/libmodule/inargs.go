package libmodule

import (
	"encoding/json"
	"fmt"
	"os"
)

// ReadModuleInput reads stdin and decodes into ModuleInput with
// the arguments struct. To use this function, you need to define
// the type of T that matches your expected input structure.
// It returns an error if the JSON decoding fails.
// Example usage:
//
//	type MyArgs[T any] struct {
//		Opts      []string `json:"opts"`
//		Arguments T        `json:"arguments"`
//	}
//
//	input, err := ReadModuleInput[MyArgs]()
//	if err != nil {
//	    fmt.Println("Error reading input:", err)
//	} else {
//	    fmt.Println("Input read successfully:", input)
//	}
//
// Note: The type T must be defined in the same package or imported
//
//	in the package where you call ReadModuleInput.
//	Ensure that the JSON structure matches the expected fields in T.
//	This function is generic and can be used with any struct type.
//	The JSON input should be in the format:
//	{
//	    "opts": ["--option1", "--option2"],
//	    "arguments": {
//	        "field1": "value1",
//	        "field2": 42
//	    }
//	}
//	where "arguments" matches the fields of type T.
//	The "opts" field is an array of strings representing command-line options.
//	The "arguments" field is a JSON object that matches the structure of T.
func ReadModuleInput[T any]() (T, error) {
	var input T
	if err := json.NewDecoder(os.Stdin).Decode(&input); err != nil {
		return input, fmt.Errorf("JSON decode error: %w", err)
	}
	return input, nil
}
