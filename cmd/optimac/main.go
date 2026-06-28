package main

import (
	"fmt"
	"os"

	"github.com/agungdp150/optimac/internal/cli"
)

var version = "0.1.2"

func main() {
	if err := cli.Run(os.Args[1:], version); err != nil {
		fmt.Fprintln(os.Stderr, "error:", err)
		os.Exit(1)
	}
}
