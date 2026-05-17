import sqlite3
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import seed_demo_data


class SeedDemoDataTest(unittest.TestCase):
    def test_seed_populates_dashboard_data_and_is_idempotent(self) -> None:
        with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as temp_dir:
            db_path = Path(temp_dir) / "forgerust.db"

            first = seed_demo_data.seed_database(db_path)
            second = seed_demo_data.seed_database(db_path)

            self.assertGreaterEqual(first["questions"], 28)
            self.assertEqual(first["questions"], second["questions"])
            self.assertEqual(first["sessions"], second["sessions"])
            self.assertEqual(first["records"], second["records"])
            self.assertGreaterEqual(second["sessions"], 14)
            self.assertGreaterEqual(second["records"], 80)
            self.assertGreaterEqual(second["wrong_questions"], 5)

            with sqlite3.connect(db_path) as conn:
                trend_days = conn.execute(
                    "SELECT COUNT(DISTINCT substr(created_at, 1, 10)) FROM training_records"
                ).fetchone()[0]
                mastered_tags = conn.execute(
                    """
                    SELECT COUNT(*) FROM (
                        SELECT q.tags, SUM(CASE WHEN r.is_correct = 1 THEN 1 ELSE 0 END) AS correct_n,
                               COUNT(*) AS total_n
                        FROM training_records r
                        JOIN questions q ON q.id = r.question_id
                        WHERE r.skipped = 0
                        GROUP BY q.tags
                        HAVING total_n >= 5 AND 1.0 * correct_n / total_n >= 0.8
                    )
                    """
                ).fetchone()[0]

            self.assertGreaterEqual(trend_days, 14)
            self.assertGreaterEqual(mastered_tags, 2)


if __name__ == "__main__":
    unittest.main()
