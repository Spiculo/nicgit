-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Wording style per account (decision N61): GitHub conventions or architectural conventions.
ALTER TABLE users ADD COLUMN style text NOT NULL DEFAULT 'git' CHECK (style IN ('git', 'architecture'));
