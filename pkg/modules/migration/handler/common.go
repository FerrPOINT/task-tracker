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
	"net/http"

	"github.com/FerrPOINT/task-tracker/pkg/modules/migration"
	user2 "github.com/FerrPOINT/task-tracker/pkg/user"
	"github.com/labstack/echo/v5"
)

func status(ms migration.MigratorName, c *echo.Context) error {
	user, err := user2.GetCurrentUser(c)
	if err != nil {
		return err
	}

	status, err := migration.GetMigrationStatus(ms, user)
	if err != nil {
		return err
	}

	return c.JSON(http.StatusOK, status)
}
