from auth import login, logout
from models import User

def main():
    user = login("admin", "secret")
    if user:
        print(f"Logged in: {user.name}")
        logout(user)

if __name__ == "__main__":
    main()
