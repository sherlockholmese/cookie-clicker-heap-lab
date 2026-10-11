**Overview – Cookie Clicker Pwn Workshop**  
This project is meant to be a simple Cookie Clicker style game that teaches students the fundamentals of binary exploitation. A worksheet handout will be given alongside the game that guides students through the main teaching points intended.

**Game**  
Players will start off at a **cookie-count** of 0 cookies, and be able to click a cookie that produces 1 cookie per click. With 10 cookies, players can buy a Cursor object that produces 1 cookie per second.  The first goal of the game is to buy a **flag** worth 1 trillion cookies, and the second goal of the game is to execute a **win** function. 

Players can change their **username,** and also modify the **description** and the **name** of the Cursor objects.

**Features**  
Aside from the main game, the core functionality of the game is a built-in **debugger /** **memory view**, allowing students to read and update memory to visualize how the game and memory are related. The memory view shall be a separate, draggable window from the main game with a few key features:

* **Inputting non-ASCII values into text fields**  
  * When a text field interface is used, users should be able to input raw hex values with the ‘\\x’ notation (e.g. ‘\\x38’ indicates the raw byte with value 0x38)  
* **Memory hexdump view**.  
  * The **default** view should first be bytewise, two columns of 8 bytes each in chunks. A togglable button should trigger the view to present the bytes as packed lendian qwords instead.  
    * In this view, **pointers** will be highlighted in red, and **structs** will be underlined with individual fields highlighted in different colors as well.  
    * When hovering over a **struct field,** the **name, type** and **value** (according to the type) of the struct should appear.  
    * When hovering over a **pointer**, the value pointed to should appear.  
  * An ASCII view shall be next to it.  
  * The memory view shall allow:  
    * **reading/writing** to individual bytes.  
    * **jumping** to specific memory addresses.  
    * **dynamically** updating according to the changes made in the game, and vice-versa.  
    * **error-handling** when pointers are corrupted (details tbd).

* **Pointer view (in game)**  
  * When toggled, when the mouse hovers over **specific objects** in the game, a pointer to the value stored there should appear. I.e., if hovering over the **username,** then the pointer to the username string should appear.  
  * Clicking on this tooltip should take the student to the relevant area in the **memory view**.

**Implementation details**  
The virtual memory space where all the game's objects live shall be an entirely **virtualized** memory space (i.e., not the actual program's memory space). It should begin at 0x055200000 to simulate a realistic heap address.

The virtual memory space will be split up into different **sections**, .text for code, .bss for pointer tables and other game data. In .text, functions will be represented as bytecode (we will not be covering asm, so a faithful representation of assembly is not necessary. What is necessary is clearly labelled & highlighted function pointers with names that students can refer to).

The program will make heavy usage of **pointers** and **structs.** There will be a main **pointer table** in the game in .bss that is a list of pointers to different important objects, such as the global Cookies struct, the User struct, the individual Cursor structs. Cursors will be editable, this will be the main primitive by which the program will be pwned.

For simplification purposes, all struct fields are qword-sized.

For example, the Cookies struct will just be a single **int** *cookies*, and the User struct will be an **int** *id* and a **char\*** *username.* A *cursor* struct will be an **char\[8\]** *name*, a **char\*** *description* and a **function pointer** *on\_clicked().* The default value of the *on\_clicked()* ptr will be a simple function stored in .text *add\_cookie()* that just increments the cookie count by one. *Cursors* will be stored in an array elsewhere in .bss.

**Workshop details (flow)**  
**This section is provided only to share details of what should be possible within the game, the workshop itself will be carried out via human guidance in person. The game itself should not have a "walkthrough" that shows this workshop.** The goal of the workshop is to provide students with a clear intuition for how memory interacts with processes. The flow of the workshop will proceed as follows, alongside a worksheet:

1. Interacting with the main game. Clicking cookies, showing that the "flag" goal is unreachable, and changing their username.  
2. Introducing the **debugger / memory view.**   
   1. Clicking a cookie, showing that the memory view is updated when the cookie is clicked, and changing that specific changed byte within the memory view and showing that the game reflects this change in the cookie count.  
   2. Changing the username, and then showing that the memory view is updated.   
   3. Sub-goal: Once students understand how to make use of the debugger / memory view to modify variable values, students will have to follow a lab worksheet that walks them through attaining a 1 trillion cookie count.  
3. Briefly touch on **datatypes.**  
   1. Quick discussion on endianness (this will be covered repeatedly throughout the lesson so students don't get confused about endianness)  
   2. Sub-goal: Students complete a lab asking them to obtain the numerical values and string values of addresses in memory  
4. Briefly touch on **structs.**  
   1. Sub-goal: Students complete a lab that asks them to translate struct instances into their representations in memory, and then modify an existing struct using their answer in the game itself.  
   2.   
5. Deep dive into **pointers**.  
   1. Exploring the global **pointer table**, hovering over them and showing that they all point to different relevant values in the game.  
   2. Difference between modifying the reference & the value. Show what happens when we directly corrupt pointers, i.e. corrupting the Username string to point to a different string, and seeing that the corruption is visible in game.  
   3. Show what happens when the value of the Username field is changed from within the game and within memory also. Demonstrate that the pointer does not change.  
   4. Show what happens when we corrupt a pointer to a nonsense value. The program should trigger a "segmentation fault", show an error message, and then revert back to the original state.  
   5. Show **pointer tracing**.   
      1. Starting from .bss, we follow the global **User** pointer, click it and we are transported to a different area in .bss that shows the labelled User struct.  
      2. Then, we follow the global **Cursor array** pointer. We click the pointer and it transports us to the array of cursor pointers. We show that a pointer is **added** to this array when we buy another cursor. We show that clicking the **Cursor** pointer takes us to a **Cursor** object, which shows the id, description pointer, and a function pointer. Clicking the function pointer takes us to the .text section.  
   6. Sub-goal: when students understand how pointers interface with operations in the game, walk students through a lab guiding them to the process of modifying pointers to point to the cookie count variable such as by faking a struct in memory and modifying pointers to point to that struct, showing that writing to that pointer through means other than the debugger view can indirectly corrupt the cookie count as well.  
6. Understanding **control flow**  
   1. Exploring the function pointer table, each entry shows the address of the function and the function identifier  
   2. Showing that functions are also data in memory with addresses.  
   3. Show what happens when pointers to functions that are used in the game are modified.  
   4. Sub-goal: Students need to look for a “win” function in the function pointer table, and find a way to call it to trigger the win ending screen, which cannot be obtained otherwise.

**Pwning the game**  
**This section is provided only to share details of what should be possible within the game, the workshop itself will be carried out via human guidance in person. The game itself should not have a "walkthrough" that shows this workshop.**   
After students attain a sufficiently good understanding of pointers, structs, datatypes, function pointers, and methods, then we can finally try to pwn the game with only the game itself. The main primitive here will be **buffer overflows.** Students will use the debugger view to visualize the steps of their exploit, but not for any other purposes (i.e. any memory corruption must be done through the game).

First goal: Modifying the cookie count by a buffer overflow in the username field

1. Students use the the debugger view to modify the *max\_field\_size* value, allowing them to input an arbitrarily large amount of character into the name field  
2. This allows them to overflow into the cookie count field, modifying it to 1 trillion.

Second goal: **calling the win function** by corrupting the Cursor object.

1. Open the Cursor edit menu & the view to the Cursor struct side by side, and then modify the **name** of the cursor.  
2. Show that the memory view changes. Then, change the name to be some large string longer than 8 characters.  
3. Naturally, this will corrupt the adjacent **description** pointer and trigger a simulated segfault.  
4. The student must insert a value that will corrupt the **description** pointer to point to the Cursor's **function pointer field** instead.  
5. Then, the student will write a new **function pointer** to the description by modifying the **description** directly.  
6. Then, the **win** function should trigger.

**Auxiliary specifications**  
A few important details

* Cross-platform capabilities.  
* In terms of aesthetics, **no aesthetic decisions** aside from the ones set out in the spec are to be made by an AI, this includes color scheme, font choice, images. Use placeholder images if needed, and place all game strings in a separate file which a human can rewrite. Layout decisions are fine. The generated output should be as **aesthetically simple** and importantly as **customizable** as possible, humans will be making all aesthetic choices re: the game.  
* 

