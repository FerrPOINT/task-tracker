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

package webtests

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/FerrPOINT/task-tracker/pkg/models"
	"github.com/FerrPOINT/task-tracker/pkg/modules/auth"
	apiv1 "github.com/FerrPOINT/task-tracker/pkg/routes/api/v1"
	"github.com/FerrPOINT/task-tracker/pkg/web/handler"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestSessions(t *testing.T) {
	t.Run("List sessions for user", func(t *testing.T) {
		testHandler := webHandlerTest{
			user: &testuser1,
			strFunc: func() handler.CObject {
				return &models.Session{}
			},
			t: t,
		}
		rec, err := testHandler.testReadAllWithUser(nil, nil)
		require.NoError(t, err)
		body := rec.Body.String()
		// User 1 should see their own sessions (session 1 and 2)
		assert.Contains(t, body, "550e8400-e29b-41d4-a716-446655440001")
		assert.Contains(t, body, "550e8400-e29b-41d4-a716-446655440002")
		// User 1 should NOT see user 2's session
		assert.NotContains(t, body, "550e8400-e29b-41d4-a716-446655440003")
	})

	t.Run("Delete own session", func(t *testing.T) {
		testHandler := webHandlerTest{
			user: &testuser1,
			strFunc: func() handler.CObject {
				return &models.Session{}
			},
			t: t,
		}
		rec, err := testHandler.testDeleteWithUser(nil, map[string]string{"session": "550e8400-e29b-41d4-a716-446655440002"})
		require.NoError(t, err)
		assert.Equal(t, http.StatusOK, rec.Code)
	})

	t.Run("Cannot delete other user's session", func(t *testing.T) {
		testHandler := webHandlerTest{
			user: &testuser1,
			strFunc: func() handler.CObject {
				return &models.Session{}
			},
			t: t,
		}
		_, err := testHandler.testDeleteWithUser(nil, map[string]string{"session": "550e8400-e29b-41d4-a716-446655440003"})
		require.Error(t, err)
		assert.Equal(t, http.StatusForbidden, getHTTPErrorCode(err))
	})

	t.Run("Refresh with valid token", func(t *testing.T) {
		e, err := setupTestEnv()
		require.NoError(t, err)

		req := httptest.NewRequest(http.MethodPost, "/api/v1/user/token/refresh", strings.NewReader(""))
		req.Header.Set("Content-Type", "application/json")
		req.AddCookie(&http.Cookie{
			Name:  auth.RefreshTokenCookieName,
			Value: "testtoken_session1",
		})
		rec := httptest.NewRecorder()
		c := e.NewContext(req, rec)

		err = apiv1.RefreshToken(c)
		require.NoError(t, err)
		assert.Equal(t, http.StatusOK, rec.Code)
		assert.Contains(t, rec.Body.String(), "token")
	})

	t.Run("Refresh with invalid token", func(t *testing.T) {
		e, err := setupTestEnv()
		require.NoError(t, err)

		req := httptest.NewRequest(http.MethodPost, "/api/v1/user/token/refresh", strings.NewReader(""))
		req.Header.Set("Content-Type", "application/json")
		req.AddCookie(&http.Cookie{
			Name:  auth.RefreshTokenCookieName,
			Value: "garbage",
		})
		rec := httptest.NewRecorder()
		c := e.NewContext(req, rec)

		err = apiv1.RefreshToken(c)
		require.Error(t, err)
		assert.Equal(t, http.StatusUnauthorized, getHTTPErrorCode(err))
	})

	t.Run("Login creates session", func(t *testing.T) {
		e, err := setupTestEnv()
		require.NoError(t, err)

		req := httptest.NewRequest(http.MethodPost, "/api/v1/login", strings.NewReader(`{
  "username": "user1",
  "password": "12345678"
}`))
		req.Header.Set("Content-Type", "application/json")
		rec := httptest.NewRecorder()
		c := e.NewContext(req, rec)

		err = apiv1.Login(c)
		require.NoError(t, err)
		assert.Equal(t, http.StatusOK, rec.Code)
		assert.Contains(t, rec.Body.String(), "token")

		// Check that a Set-Cookie header with the refresh token cookie was set
		cookies := rec.Result().Cookies()
		var foundRefreshCookie bool
		for _, cookie := range cookies {
			if cookie.Name == auth.RefreshTokenCookieName {
				foundRefreshCookie = true
				assert.NotEmpty(t, cookie.Value)
				assert.True(t, cookie.HttpOnly)
				break
			}
		}
		assert.True(t, foundRefreshCookie, "Expected a Set-Cookie header with the refresh token cookie")
	})
}
