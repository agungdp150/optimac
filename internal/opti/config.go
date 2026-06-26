package opti

import (
	"encoding/json"
	"os"
	"path/filepath"
)

// Config holds user-tunable behaviour loaded from
// ~/.config/opti-mac/config.json. A missing file yields DefaultConfig.
type Config struct {
	// UseTrash moves cleaned files to the OptiMac trash instead of deleting
	// them permanently, so they can be restored. Defaults to true.
	UseTrash bool `json:"use_trash"`
	// ExtraCleanTargets are appended to the built-in deep-clean targets.
	ExtraCleanTargets []ConfigTarget `json:"extra_clean_targets"`
	// ExcludePaths are skipped during cleaning even if a target matches them.
	// Each entry is matched as a prefix against the absolute item path and may
	// use ~ for the home directory.
	ExcludePaths []string `json:"exclude_paths"`
}

// ConfigTarget is a user-defined cleanup root.
type ConfigTarget struct {
	Path     string `json:"path"`
	Kind     string `json:"kind"`
	Category string `json:"category"`
	Sudo     bool   `json:"sudo"`
}

// DefaultConfig returns the configuration used when no file is present.
func DefaultConfig() Config {
	return Config{UseTrash: true}
}

// ConfigPath returns the absolute path to the OptiMac config file.
func ConfigPath() (string, error) {
	dir, err := configDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(dir, "config.json"), nil
}

func configDir() (string, error) {
	home, err := HomeDir()
	if err != nil {
		return "", err
	}
	if base := os.Getenv("XDG_CONFIG_HOME"); base != "" && os.Geteuid() != 0 {
		return filepath.Join(base, "opti-mac"), nil
	}
	return filepath.Join(home, ".config", "opti-mac"), nil
}

// LoadConfig reads the config file, returning DefaultConfig if it is absent.
func LoadConfig() (Config, error) {
	path, err := ConfigPath()
	if err != nil {
		return DefaultConfig(), err
	}
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return DefaultConfig(), nil
		}
		return DefaultConfig(), err
	}
	cfg := DefaultConfig()
	if err := json.Unmarshal(data, &cfg); err != nil {
		return DefaultConfig(), err
	}
	return cfg, nil
}

// SaveConfig writes the config file, creating the directory if needed.
func SaveConfig(cfg Config) (string, error) {
	dir, err := configDir()
	if err != nil {
		return "", err
	}
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return "", err
	}
	path := filepath.Join(dir, "config.json")
	data, err := json.MarshalIndent(cfg, "", "  ")
	if err != nil {
		return "", err
	}
	if err := os.WriteFile(path, append(data, '\n'), 0o644); err != nil {
		return "", err
	}
	return path, nil
}

// excluded reports whether the given absolute path is covered by any configured
// exclude prefix.
func (c Config) excluded(path string) bool {
	clean := filepath.Clean(path)
	for _, raw := range c.ExcludePaths {
		expanded, err := ExpandPath(raw)
		if err != nil || expanded == "" {
			continue
		}
		expanded = filepath.Clean(expanded)
		if clean == expanded || hasPathPrefix(clean, expanded) {
			return true
		}
	}
	return false
}

func hasPathPrefix(path, prefix string) bool {
	if prefix == "" {
		return false
	}
	if path == prefix {
		return true
	}
	sep := string(os.PathSeparator)
	if prefix[len(prefix)-1:] == sep {
		return len(path) > len(prefix) && path[:len(prefix)] == prefix
	}
	return len(path) > len(prefix) && path[:len(prefix)+1] == prefix+sep
}
