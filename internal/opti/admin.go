package opti

import (
	"fmt"
	"os"
	"os/user"
	"runtime"
	"strings"
)

// RunDeepCleanWithAdmin executes the CLI deep-clean path through macOS
// administrator authorization so sudo-only targets are included without
// breaking the full-screen terminal UI with a password prompt.
func RunDeepCleanWithAdmin() (string, error) {
	if runtime.GOOS != "darwin" {
		return "", fmt.Errorf("admin deep clean is only supported on macOS")
	}
	executable, err := os.Executable()
	if err != nil {
		return "", err
	}
	command := elevatedCleanCommand(executable, originalUserName())
	if os.Geteuid() == 0 {
		return runCommand(executable, "clean", "--execute", "--sudo")
	}
	script := fmt.Sprintf("do shell script %s with administrator privileges", appleScriptString(command))
	output, err := runCommand("osascript", "-e", script)
	if err != nil {
		message := strings.TrimSpace(output)
		if message == "" {
			message = err.Error()
		}
		return output, fmt.Errorf("admin deep clean failed: %s", message)
	}
	return output, nil
}

func elevatedCleanCommand(executable, username string) string {
	args := []string{"env", "OPTI_MAC_ELEVATED=1"}
	if username != "" {
		args = append(args, "OPTI_MAC_USER="+username, "SUDO_USER="+username)
	}
	args = append(args, executable, "clean", "--execute", "--sudo")
	quoted := make([]string, 0, len(args))
	for _, arg := range args {
		quoted = append(quoted, shellQuote(arg))
	}
	return strings.Join(quoted, " ")
}

func originalUserName() string {
	if value := os.Getenv("OPTI_MAC_USER"); value != "" && value != "root" {
		return value
	}
	if value := os.Getenv("SUDO_USER"); value != "" && value != "root" {
		return value
	}
	if value := os.Getenv("USER"); value != "" && value != "root" {
		return value
	}
	if current, err := user.Current(); err == nil && current.Username != "" && current.Username != "root" {
		name := current.Username
		if idx := strings.LastIndex(name, "\\"); idx >= 0 {
			name = name[idx+1:]
		}
		return name
	}
	return ""
}

func shellQuote(value string) string {
	if value == "" {
		return "''"
	}
	return "'" + strings.ReplaceAll(value, "'", `'\''`) + "'"
}

func appleScriptString(value string) string {
	value = strings.ReplaceAll(value, `\`, `\\`)
	value = strings.ReplaceAll(value, `"`, `\"`)
	return `"` + value + `"`
}
