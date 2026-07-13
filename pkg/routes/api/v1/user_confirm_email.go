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

package v1

import (
	"net/http"

	"github.com/FerrPOINT/task-tracker/pkg/models"
	"github.com/FerrPOINT/task-tracker/pkg/routes/api/shared"
	"github.com/FerrPOINT/task-tracker/pkg/user"
	"github.com/labstack/echo/v5"
)

// UserConfirmEmail is the handler to confirm a user email
// @Summary Confirm the email of a new user
// @Description Confirms the email of a newly registered user.
// @tags user
// @Accept json
// @Produce json
// @Param credentials body user.EmailConfirm true "The token."
// @Success 200 {object} models.Message
// @Failure 412 {object} web.HTTPError "Bad token provided."
// @Failure 500 {object} models.Message "Internal error"
// @Router /user/confirm [post]
func UserConfirmEmail(c *echo.Context) error {
	// Check for Request Content
	var emailConfirm user.EmailConfirm
	if err := c.Bind(&emailConfirm); err != nil {
		return echo.NewHTTPError(http.StatusBadRequest, "No token provided.").Wrap(err)
	}

	if err := shared.ConfirmEmail(&emailConfirm); err != nil {
		return err
	}

	return c.JSON(http.StatusOK, models.Message{Message: "The email was confirmed successfully."})
}
