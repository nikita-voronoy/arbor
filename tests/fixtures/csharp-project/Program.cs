using System;

namespace MyApp
{
    public class Program
    {
        public static void Main(string[] args)
        {
            var auth = new AuthService();
            var user = auth.Login("admin", "secret");
            if (user != null)
            {
                Console.WriteLine($"Welcome, {user.Name}!");
                auth.Logout(user);
            }
        }
    }
}
