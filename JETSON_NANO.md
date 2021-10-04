# Jetson Nano

## Setup
My hardware setup is:
1. NVIDIA Jetson Nano (4 Gb RAM)- https://developer.nvidia.com/embedded/jetson-nano-developer-kit
2. Power supply via DC barrel jack. Input 100-240VAC, 50/60Hz, 0.6A. Output 5V 4A, 20W MAX. Do not forget a jumper pin (it tells the Nano to use DC barrel jack instead of micro-USB)
3. FAN-4020-PWM-5V
4. SanDisk Ultra microSDXC UHS-1 Card 64Gb

My software setup is:
1. Jetson Nano image by NVIDIA: https://developer.download.nvidia.com/embedded/L4T/r32_Release_v6.1/Jeston_Nano/jetson-nano-jp46-sd-card-image.zip

    Follow official [instruction](https://developer.nvidia.com/embedded/learn/get-started-jetson-nano-devkit#write) for OS installation.
2. [OpenCV](https://opencv.org/) v4.5.3.

    You can use this [Makefile](Makefile) for installation
    ```shell
    sudo make install_jetson
    ```
3. Programming language - [Rust](https://www.rust-lang.org/)

    Follow official [instruction](https://www.rust-lang.org/tools/install)