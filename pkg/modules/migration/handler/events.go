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

package handler

import (
	"github.com/FerrPOINT/task-tracker/pkg/user"
)

// MigrationRequestedEvent represents a MigrationRequestedEvent event
type MigrationRequestedEvent struct {
	Migrator     interface{} `json:"migrator"`
	User         *user.User  `json:"user"`
	MigratorKind string      `json:"migrator_kind"`
}

// Name defines the name for MigrationRequestedEvent
func (t *MigrationRequestedEvent) Name() string {
	return "migration.requested"
}
