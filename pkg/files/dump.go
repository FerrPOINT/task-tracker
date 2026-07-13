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

package files

import (
	"errors"
	"io"
	gofs "io/fs"
)

// Dump dumps all saved files
// This only includes the raw files, no db entries.
func Dump() (allFiles map[int64]io.ReadCloser, err error) {
	files := []*File{}
	err = x.Find(&files)
	if err != nil {
		return
	}

	allFiles = make(map[int64]io.ReadCloser, len(files))
	for _, file := range files {
		err = file.LoadFileByID()
		if err != nil {
			var pathError *gofs.PathError
			if errors.As(err, &pathError) {
				continue
			}
			return
		}
		allFiles[file.ID] = file.File
	}

	return
}
