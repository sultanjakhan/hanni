# Module 11: Media

## Description

Hobbies and media collections covering 9 content types (Music, Anime, Manga, Movies, Series, Cartoons, Games, Books, Podcasts). Supports statuses, ratings, progress tracking, and user-defined lists. Integrates with chat for adding media via actions.

## Overview

| Attribute        | Value                                              |
|------------------|----------------------------------------------------|
| Domain           | Hobbies, media collections (9 types), user lists   |
| Total LOC        | ~460                                               |
| Backend          | `lib.rs` (L4457-4521, L4800-5012)                |
| Frontend         | `main.js` (L4594-4773)                            |
| Backend functions | 4                                                 |
| Frontend functions | 3                                               |
| Complexity       | Simple: 3, Medium: 4, Complex: 0                  |

## Files

| File                              | Lines          | Role                            |
|-----------------------------------|----------------|---------------------------------|
| `desktop/src-tauri/src/lib.rs`    | L4457-4521     | Hobbies CRUD                     |
| `desktop/src-tauri/src/lib.rs`    | L4800-5012     | Media items, user lists, stats   |
| `desktop/src/main.js`             | L4594-4773     | Media UI (overview, list, modal) |

## Dependencies

| Direction | Module    | Relationship                                    |
|-----------|-----------|-------------------------------------------------|
| Uses      | core      | DB access for all CRUD operations                |
| Used by   | chat      | Action execution adds media items                |
| Used by   | lifestyle | Hobbies widget references hobby data             |

## Key Concepts

- **9 Media Types**: Music, Anime, Manga, Movies, Series, Cartoons, Games, Books, Podcasts — each with status, rating, and progress fields.
- **Hobbies**: Separate entity with entry logging (hours, notes).
- **User Lists**: Custom named lists that can hold any media items. Add/remove items freely.
- **Hide/Unhide**: Media items can be hidden without deletion.
- **Media Stats**: Aggregated statistics across all media types.
