package opti

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

// validateRemovable resolves path to an absolute, cleaned form and confirms it
// is neither protected nor outside the allowed cleanup roots.
func validateRemovable(path string, allowedRoots []string) (string, error) {
	cleanPath, err := filepath.Abs(path)
	if err != nil {
		return "", err
	}
	cleanPath = filepath.Clean(cleanPath)

	if isProtectedPath(cleanPath) {
		return "", fmt.Errorf("refusing to remove protected path: %s", cleanPath)
	}

	allowed := false
	for _, root := range allowedRoots {
		expanded, err := ExpandPath(root)
		if err != nil {
			return "", err
		}
		expanded = filepath.Clean(expanded)
		if cleanPath != expanded && strings.HasPrefix(cleanPath, expanded+string(os.PathSeparator)) {
			allowed = true
			break
		}
	}
	if !allowed {
		return "", fmt.Errorf("refusing to remove path outside allowed cleanup roots: %s", cleanPath)
	}
	return cleanPath, nil
}

// RemoveAllSafe permanently removes path after validating it against the allowed
// cleanup roots and the protected-path list.
func RemoveAllSafe(path string, allowedRoots []string) error {
	cleanPath, err := validateRemovable(path, allowedRoots)
	if err != nil {
		return err
	}
	if _, err := os.Lstat(cleanPath); err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil
		}
		return err
	}
	return os.RemoveAll(cleanPath)
}

// TrashSafe validates path the same way as RemoveAllSafe, then moves it into the
// supplied trash session instead of deleting it.
func TrashSafe(path string, allowedRoots []string, session *TrashSession, size int64) error {
	cleanPath, err := validateRemovable(path, allowedRoots)
	if err != nil {
		return err
	}
	if _, err := os.Lstat(cleanPath); err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil
		}
		return err
	}
	return session.Trash(cleanPath, size)
}

func isProtectedPath(path string) bool {
	clean := filepath.Clean(path)
	protectedExact := map[string]bool{
		"/":             true,
		"/Applications": true,
		"/Library":      true,
		"/System":       true,
		"/Users":        true,
		"/bin":          true,
		"/etc":          true,
		"/private":      true,
		"/sbin":         true,
		"/tmp":          true,
		"/usr":          true,
		"/var":          true,
	}
	home, err := HomeDir()
	if err == nil {
		for _, path := range []string{
			home,
			filepath.Join(home, "Desktop"),
			filepath.Join(home, "Documents"),
			filepath.Join(home, "Downloads"),
			filepath.Join(home, ".zlogin"),
			filepath.Join(home, ".zlogout"),
			filepath.Join(home, ".zprofile"),
			filepath.Join(home, ".zshenv"),
			filepath.Join(home, ".zshrc"),
			filepath.Join(home, ".p10k.zsh"),
			filepath.Join(home, ".config", "starship.toml"),
			filepath.Join(home, ".zcompdump"),
		} {
			protectedExact[path] = true
		}
		for _, path := range []string{
			filepath.Join(home, ".ssh"),
			filepath.Join(home, ".zsh"),
			filepath.Join(home, ".oh-my-zsh"),
			filepath.Join(home, ".zprezto"),
			filepath.Join(home, ".zim"),
			filepath.Join(home, ".zinit"),
			filepath.Join(home, ".antigen"),
			filepath.Join(home, ".antidote"),
			filepath.Join(home, ".poshthemes"),
			filepath.Join(home, ".config", "zsh"),
			filepath.Join(home, ".config", "oh-my-posh"),
			filepath.Join(home, ".config", "starship"),
			filepath.Join(home, ".cache", "oh-my-zsh"),
			filepath.Join(home, ".cache", "starship"),
			filepath.Join(home, ".cache", "p10k"),
			filepath.Join(home, ".cache", "powerlevel10k"),
			filepath.Join(home, ".cache", "zsh"),
			filepath.Join(home, ".cache", "zinit"),
			filepath.Join(home, ".cache", "antidote"),
			filepath.Join(home, ".cache", "antigen"),
			filepath.Join(home, ".local", "share", "zinit"),
			filepath.Join(home, ".local", "share", "oh-my-zsh"),
			filepath.Join(home, ".local", "share", "zsh"),
		} {
			if clean == path || hasPathPrefix(clean, path) {
				return true
			}
		}
	}
	return protectedExact[clean]
}
