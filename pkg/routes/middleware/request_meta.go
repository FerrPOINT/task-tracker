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

package middleware

import (
	"github.com/FerrPOINT/task-tracker/pkg/events"

	"github.com/labstack/echo/v5"
)

// RequestMeta stashes IP, User-Agent and the request ID on the request
// context so events dispatched while handling the request carry them as
// message metadata (consumed by the audit listeners). Must run after the
// RequestID middleware, which guarantees the response header is populated.
func RequestMeta() echo.MiddlewareFunc {
	return func(next echo.HandlerFunc) echo.HandlerFunc {
		return func(c *echo.Context) error {
			req := c.Request()
			ctx := events.WithRequestMeta(req.Context(), &events.RequestMeta{
				IP:        c.RealIP(),
				UserAgent: req.UserAgent(),
				RequestID: c.Response().Header().Get(echo.HeaderXRequestID),
			})
			c.SetRequest(req.WithContext(ctx))
			return next(c)
		}
	}
}
