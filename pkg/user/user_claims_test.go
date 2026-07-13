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

package user

import (
	"testing"

	"github.com/golang-jwt/jwt/v5"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestGetUserFromClaims_IsAdmin(t *testing.T) {
	claims := jwt.MapClaims{
		"id":       float64(1),
		"username": "u1",
		"is_admin": true,
	}
	u, err := GetUserFromClaims(claims)
	require.NoError(t, err)
	assert.True(t, u.IsAdmin)
}

func TestGetUserFromClaims_IsAdminMissing(t *testing.T) {
	claims := jwt.MapClaims{
		"id":       float64(1),
		"username": "u1",
	}
	u, err := GetUserFromClaims(claims)
	require.NoError(t, err)
	assert.False(t, u.IsAdmin)
}
