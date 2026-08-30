from typing import Generic, TypedDict, TypeVar

T = TypeVar("T")


class Pagination(TypedDict):
    cursor: str | None


class ApiResponse(TypedDict, Generic[T]):
    data: list[T]
    pagination: Pagination | None


class Stream(TypedDict):
    id: str
    user_id: str
    user_login: str
    user_name: str
    game_id: str
    game_name: str
    type: str
    title: str
    viewer_count: int
    started_at: str
    language: str
    thumbnail_url: str
    tags: list[str]
    is_mature: bool


class User(TypedDict):
    id: str
    login: str
    display_name: str
    type: str
    broadcaster_type: str
    description: str
    profile_image_url: str
    offline_image_url: str
    view_count: int
    created_at: str
