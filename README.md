# SPT Server Manager

This lightweight Rust project automatically detects and runs the SPT server executable in the same directory. It allows you to interact with the server through a terminal interface.


---
## Key Features

- **Automatic Server Detection:** If the server executable is in the same directory, it will be detected and launched automatically.
- **Command Interface:** Use the `help` command to see a list of available commands in the terminal.

## Available Commands

- `help`      - Display this help message.
- `exit`      - Stop the server and exit the program.
- `restart`   - Restart the server.
- `setpath`   - Change the server executable path.
- `[command]` - Any other input will be sent to the server as a command.

## Usage
 Drop and Run the executable in the same directory as your SPT server executable.
>If you Run it from an external directory, you will be prompted to enter a path to SPT.server.exe

### Example Commands

```shell
Type 'help' for a list of available commands.
> help
```
as simple as that.

---
# License

This project is licensed under the **Creative Commons Attribution-NonCommercial-ShareAlike 4.0 International (CC BY-NC-SA 4.0)** License.

For full license terms, see the [LICENSE.md](LICENSE.md) file.
