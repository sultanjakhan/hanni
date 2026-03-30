# Work (Job Search)

## Purpose
Job search CRM — find, track, and manage job applications across multiple sources. Hanni searches for vacancies, user tracks application progress through a pipeline.

## DB Tables (planned)

| Table | Purpose |
|-------|---------|
| `job_sources` | Where to search: TG channels, websites, job boards |
| `job_roles` | What to search: desired specialties, keywords, salary range |
| `job_vacancies` | Found vacancies with company, position, salary, stage |
| `job_search_log` | History of Hanni's searches: when, where, how many found |

### job_sources
| Column | Type | Purpose |
|--------|------|---------|
| id | INTEGER | PK |
| name | TEXT | "Python Jobs TG", "hh.ru" |
| type | TEXT | `telegram`, `website`, `linkedin`, `other` |
| url | TEXT | Link to channel/site |
| active | INTEGER | Enabled for search |

### job_roles
| Column | Type | Purpose |
|--------|------|---------|
| id | INTEGER | PK |
| title | TEXT | "ML Engineer", "Python Backend" |
| keywords | TEXT | "python, pytorch, fastapi" |
| salary_min | INTEGER | Minimum acceptable salary |
| priority | TEXT | `high`, `medium`, `low` |

### job_vacancies
| Column | Type | Purpose |
|--------|------|---------|
| id | INTEGER | PK |
| company | TEXT | Company name |
| position | TEXT | Job title |
| source_id | INTEGER | → job_sources |
| role_id | INTEGER | → job_roles |
| salary | TEXT | Salary range as text |
| url | TEXT | Link to vacancy |
| stage | TEXT | Pipeline stage |
| notes | TEXT | User notes |
| found_at | TEXT | When Hanni found it |
| updated_at | TEXT | Last stage change |

### job_search_log
| Column | Type | Purpose |
|--------|------|---------|
| id | INTEGER | PK |
| source_id | INTEGER | → job_sources |
| searched_at | TEXT | When Hanni searched |
| found_count | INTEGER | How many new vacancies |
| notes | TEXT | Search summary |

## Pipeline Stages

```
found → saved → applied → responded → interview → offer → accepted
                              ↘ rejected
                              ↘ ignored
```

## Views

- **Dashboard** — stats from data: total vacancies, by stage, applications per week, conversion rate
- **Sources** — sub-tabs per source (TG channels, sites), manage active sources
- **Vacancies** — Kanban by stage OR table with filters
- **Roles** — desired specialties and keywords
- **Search Log** — when Hanni searched, what found

## Relations
- **Chat** — Hanni searches sources and adds vacancies automatically
- **Schedule** — "apply to N jobs per week" routine
- **Calendar** — interview dates appear on calendar

## Notable
- Hanni actively searches configured sources (TG channels, websites)
- Sources double as sub-tabs — each source has its own vacancy list
- Old `projects` + `tasks` tables to be removed (replaced by this)
- Dashboard analytics built from vacancy data (no separate stats table needed)

## Status
- Current: old task management (projects + tasks)
- Planned: full job search CRM (this document)
