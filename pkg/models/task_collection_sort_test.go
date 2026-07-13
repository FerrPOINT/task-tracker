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
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestSortParamValidation(t *testing.T) {
	t.Run("Test valid order by", func(t *testing.T) {
		t.Run(orderAscending.String(), func(t *testing.T) {
			s := &sortParam{
				orderBy: orderAscending,
				sortBy:  "id",
			}
			err := s.validate()
			require.NoError(t, err)
		})
		t.Run(orderDescending.String(), func(t *testing.T) {
			s := &sortParam{
				orderBy: orderDescending,
				sortBy:  "id",
			}
			err := s.validate()
			require.NoError(t, err)
		})
	})
	t.Run("Test valid sort by", func(t *testing.T) {
		for _, test := range []string{
			taskPropertyID,
			taskPropertyTitle,
			taskPropertyDescription,
			taskPropertyDone,
			taskPropertyDoneAt,
			taskPropertyDueDate,
			taskPropertyCreatedByID,
			taskPropertyProjectID,
			taskPropertyRepeatAfter,
			taskPropertyPriority,
			taskPropertyStartDate,
			taskPropertyEndDate,
			taskPropertyHexColor,
			taskPropertyPercentDone,
			taskPropertyUID,
			taskPropertyCreated,
			taskPropertyUpdated,
		} {
			t.Run(test, func(t *testing.T) {
				s := &sortParam{
					orderBy: orderAscending,
					sortBy:  test,
				}
				err := s.validate()
				require.NoError(t, err)
			})
		}
	})
	t.Run("Test invalid order by", func(t *testing.T) {
		s := &sortParam{
			orderBy: "somethingInvalid",
			sortBy:  "id",
		}
		err := s.validate()
		require.Error(t, err)
		assert.True(t, IsErrInvalidSortOrder(err))
	})
	t.Run("Test invalid sort by", func(t *testing.T) {
		s := &sortParam{
			orderBy: orderAscending,
			sortBy:  "somethingInvalid",
		}
		err := s.validate()
		require.Error(t, err)
		assert.True(t, IsErrInvalidTaskField(err))
	})
}
