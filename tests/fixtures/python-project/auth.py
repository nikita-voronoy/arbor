from models import User

def login(username: str, password: str) -> User | None:
    user = find_user(username)
    if user and verify_password(user, password):
        return user
    return None

def find_user(username: str) -> User | None:
    if username == "admin":
        return User(id=1, name="Admin", role="admin")
    return None

def verify_password(user: User, password: str) -> bool:
    return password == "secret"

def logout(user: User) -> None:
    print(f"Logging out: {user.name}")
