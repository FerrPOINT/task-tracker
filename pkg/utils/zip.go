// Task Tracker is a self-hosted task and kanban board application.
// Copyright 2026-present Task Tracker and contributors. All rights reserved.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

package utils

import (
	"path/filepath"
	"strings"
)

// ContainsPathTraversal checks if a zip entry name contains directory traversal
// sequences that could be used to write files outside the intended directory.
// This includes Unix-style traversal (../) and Windows-style absolute paths (C:\, \).
func ContainsPathTraversal(name string) bool {
	cleanPath := filepath.ToSlash(filepath.Clean(name))

	// Check for parent directory traversal
	if strings.HasPrefix(cleanPath, "../") ||
		strings.Contains(cleanPath, "/../") ||
		cleanPath == ".." {
		return true
	}

	// Check for Unix absolute paths
	if strings.HasPrefix(name, "/") {
		return true
	}

	// Check for Windows-style paths: drive letters (C:), UNC paths (\\), or leading backslash
	if strings.HasPrefix(name, "\\") || strings.Contains(name, ":\\") {
		return true
	}

	// Use filepath.IsAbs to catch any platform-specific absolute paths
	// Note: filepath.IsAbs behavior varies by platform, but we check on all platforms
	// to ensure consistent validation regardless of where the server runs
	if filepath.IsAbs(name) {
		return true
	}

	// Check for Windows volume names (e.g., "C:" without backslash)
	if filepath.VolumeName(name) != "" {
		return true
	}

	return false
}
