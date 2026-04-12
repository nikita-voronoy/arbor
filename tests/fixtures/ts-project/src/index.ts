import { login, logout } from './auth';
import { User } from './models';

function main(): void {
    const user = login("admin", "secret");
    if (user) {
        console.log(`Logged in: ${user.name}`);
        logout(user);
    }
}

main();
