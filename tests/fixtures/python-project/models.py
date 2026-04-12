from dataclasses import dataclass

@dataclass
class User:
    id: int
    name: str
    role: str

@dataclass
class Session:
    user: User
    token: str

class AuthError(Exception):
    pass
