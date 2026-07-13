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

package models

import (
	"github.com/FerrPOINT/task-tracker/pkg/web"
	"xorm.io/xorm"
)

// CanRead checks if the user can see an attachment
func (ta *TaskAttachment) CanRead(s *xorm.Session, a web.Auth) (bool, int, error) {
	t := &Task{ID: ta.TaskID}
	return t.CanRead(s, a)
}

// CanDelete checks if the user can delete an attachment
func (ta *TaskAttachment) CanDelete(s *xorm.Session, a web.Auth) (bool, error) {
	t := &Task{ID: ta.TaskID}
	return t.CanWrite(s, a)
}

// CanCreate checks if the user can create an attachment
func (ta *TaskAttachment) CanCreate(s *xorm.Session, a web.Auth) (bool, error) {
	t, err := GetTaskByIDSimple(s, ta.TaskID)
	if err != nil {
		return false, err
	}
	return t.CanCreate(s, a)
}
