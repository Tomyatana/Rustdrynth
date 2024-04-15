This is just a command line utility for installing modrinth mods.
It uses clap for the command line utility, so any help with the commands is accesible by using the -h or --help tag, but I'm going to list the commands anyway.

The commands are:

`search`, with the arguments -q(uery), the string to search for, -v for the specified version, and -c(ategories), the specified categories, the mod loader also is counted as a category.
It lists the first 10 mods found.

`info`, with the argument -p(roject), the target project. It gets the project's description.

`dependencies`, with the arguments -p(roject), the target project, -v, the target game version and -l(oader), the targeted loader. 

It Outputs the dependencies and the specific dependency type for that project's specified version and loader.


`download`, with the arguments `-p`(roject), `-v`, the game version, `-l`(oader), the targeted mod loader, and the flag `--mcdir`, which, if included, installs the mod on the mods folder.
The `--mcdir` searches the usual path where the .minecraft folder is found.
