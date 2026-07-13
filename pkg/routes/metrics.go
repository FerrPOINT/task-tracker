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

package routes

import (
	"crypto/subtle"

	"github.com/FerrPOINT/task-tracker/pkg/config"
	"github.com/FerrPOINT/task-tracker/pkg/log"
	"github.com/FerrPOINT/task-tracker/pkg/metrics"
	"github.com/FerrPOINT/task-tracker/pkg/models"
	auth2 "github.com/FerrPOINT/task-tracker/pkg/modules/auth"

	"github.com/labstack/echo/v5"
	"github.com/labstack/echo/v5/middleware"
	"github.com/prometheus/client_golang/prometheus/promhttp"
)

func setupMetrics(a *echo.Group) {
	if !config.MetricsEnabled.GetBool() {
		return
	}

	metrics.InitMetrics()

	r := a.Group("/metrics")

	if config.MetricsUsername.GetString() != "" && config.MetricsPassword.GetString() != "" {
		r.Use(middleware.BasicAuth(func(_ *echo.Context, username, password string) (bool, error) {
			if subtle.ConstantTimeCompare([]byte(username), []byte(config.MetricsUsername.GetString())) == 1 &&
				subtle.ConstantTimeCompare([]byte(password), []byte(config.MetricsPassword.GetString())) == 1 {
				return true, nil
			}
			return false, nil
		}))
	}

	r.GET("", echo.WrapHandler(promhttp.HandlerFor(metrics.GetRegistry(), promhttp.HandlerOpts{})))
}

func setupMetricsMiddleware(a *echo.Group) {
	if !config.MetricsEnabled.GetBool() {
		return
	}

	a.Use(func(next echo.HandlerFunc) echo.HandlerFunc {
		return func(c *echo.Context) error {

			// Update currently active users
			if err := updateActiveUsersFromContext(c); err != nil {
				log.Error(err)
				return next(c)
			}
			return next(c)
		}
	})
}

// updateActiveUsersFromContext updates the currently active users in redis
func updateActiveUsersFromContext(c *echo.Context) (err error) {
	auth, err := auth2.GetAuthFromClaims(c)
	if err != nil {
		return
	}

	if _, is := auth.(*models.LinkSharing); is {
		return metrics.SetLinkShareActive(auth)
	}

	return metrics.SetUserActive(auth)
}
