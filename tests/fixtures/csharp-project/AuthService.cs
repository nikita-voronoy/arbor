using System;

namespace MyApp
{
    public class AuthService : IAuthProvider
    {
        public User Login(string username, string password)
        {
            var user = FindUser(username);
            if (user != null && VerifyPassword(user, password))
            {
                return user;
            }
            return null;
        }

        public void Logout(User user)
        {
            Console.WriteLine($"User {user.Name} logged out");
        }

        private User FindUser(string username)
        {
            return new User { Name = username, Email = $"{username}@example.com" };
        }

        private bool VerifyPassword(User user, string password)
        {
            return password.Length > 0;
        }

        public User Authenticate(string username, string password)
        {
            return Login(username, password);
        }

        public void Revoke(Session session)
        {
            Console.WriteLine("Session revoked");
        }
    }
}
