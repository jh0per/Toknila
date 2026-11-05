# Tasks: Sample Feature

Free text before the first phase is ignored.

## Phase 1: Setup

- [x] T001 Confirm the accepted scope in `specs/001-sample-feature/spec.md`
- [x] T002 [P] Record the design procedures in `docs/design.md`

## Phase 2: Foundational State

- [x] T003 Implement independent loading, error, retry states in `src/state.rs`
- [x] T004 Add the store module

## Phase 3: User Story 1 — Choose a creation path

**Goal**: A person picks how to start.
**Independent Test**: Open the home page and see the choices.

- [x] T005 [US1] Build the navigation and hero in `src/home.rs`
- [ ] T006 [P] [US1] Implement the hero wall in `src/wall.rs`

## Phase 4: User Story 2 — Resume a draft

**Goal**: A person continues where they stopped.

- [ ] T007 [US2] Load the last draft in `src/draft.rs`
- [ ] T008 [P] [US2] Show the draft badge in `src/badge.rs`

## Phase 5: User Story 3 - Share a result

- [ ] T009 [US3] Add the share sheet

## Phase 6: Polish, Integration, and Release Evidence

- [ ] T010 Record the release evidence
- [ ] T011 [P] Update the docs

## Dependencies

- Phase 2 blocks every user story.
- Stories are independent of each other.

## Parallel Opportunities

- T002 can run beside T001.

## Implementation Strategy

Ship story 1 first, then the rest.
