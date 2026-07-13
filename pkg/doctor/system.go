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

package doctor

import (
	"fmt"
	"os"
	"runtime"

	"github.com/FerrPOINT/task-tracker/pkg/version"
)

// CheckSystem returns system information checks.
func CheckSystem() CheckGroup {
	results := []CheckResult{
		checkVersion(),
		checkGoVersion(),
		checkOS(),
		checkUser(),
		checkWorkingDirectory(),
		checkUserNamespace(),
	}

	return CheckGroup{
		Name:    "System",
		Results: results,
	}
}

func checkVersion() CheckResult {
	return CheckResult{
		Name:   "Version",
		Passed: true,
		Value:  version.Version,
	}
}

func checkGoVersion() CheckResult {
	return CheckResult{
		Name:   "Go",
		Passed: true,
		Value:  runtime.Version(),
	}
}

func checkOS() CheckResult {
	return CheckResult{
		Name:   "OS",
		Passed: true,
		Value:  fmt.Sprintf("%s/%s", runtime.GOOS, runtime.GOARCH),
	}
}

func checkWorkingDirectory() CheckResult {
	wd, err := os.Getwd()
	if err != nil {
		return CheckResult{
			Name:   "Working directory",
			Passed: false,
			Error:  err.Error(),
		}
	}

	return CheckResult{
		Name:   "Working directory",
		Passed: true,
		Value:  wd,
	}
}
