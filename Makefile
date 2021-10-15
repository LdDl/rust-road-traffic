.ONESHELL:
.PHONY: test deps download build clean

# OpenCV version to use.
OPENCV_VERSION?=4.5.3

# Temporary directory to put files into.
TMP_DIR?=/tmp/

# Build shared or static library
BUILD_SHARED_LIBS?=ON

# Package list for each well-known Linux distribution
RPMS=cmake curl wget git gtk2-devel libpng-devel libjpeg-devel libtiff-devel tbb tbb-devel libdc1394-devel unzip gcc-c++
DEBS=unzip wget build-essential cmake curl git libgtk2.0-dev pkg-config libavcodec-dev libavformat-dev libswscale-dev libtbb2 libtbb-dev libjpeg-dev libpng-dev libtiff-dev libdc1394-22-dev
JETSON=build-essential cmake git unzip pkg-config libjpeg-dev libpng-dev libtiff-dev libavcodec-dev libavformat-dev libswscale-dev libgtk2.0-dev libcanberra-gtk* libxvidcore-dev libx264-dev libgtk-3-dev libtbb2 libtbb-dev libdc1394-22-dev libv4l-dev v4l-utils libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libavresample-dev libvorbis-dev libxine2-dev libfaac-dev libmp3lame-dev libtheora-dev libopencore-amrnb-dev libopencore-amrwb-dev libopenblas-dev libatlas-base-dev libblas-dev liblapack-dev libeigen3-dev gfortran libhdf5-dev protobuf-compiler libprotobuf-dev libgoogle-glog-dev libgflags-dev

# Detect Linux distribution
distro_deps=
ifneq ($(shell which dnf 2>/dev/null),)
	distro_deps=deps_fedora
else
ifneq ($(shell which apt-get 2>/dev/null),)
	distro_deps=deps_debian
else
ifneq ($(shell which yum 2>/dev/null),)
	distro_deps=deps_rh_centos
endif
endif
endif

# Install all necessary dependencies.
deps: $(distro_deps)

deps_rh_centos:
	sudo yum -y install pkgconfig $(RPMS)

deps_fedora:
	sudo dnf -y install pkgconf-pkg-config $(RPMS)

deps_debian:
	sudo apt-get -y update
	sudo apt-get -y install $(DEBS)

deps_jetson:
	sudo sh -c "echo '/usr/local/cuda/lib64' >> /etc/ld.so.conf.d/nvidia-tegra.conf"
	sudo ldconfig
	sudo apt-get -y update
	sudo apt-get -y install $(JETSON)

# Download OpenCV source tarballs.
download:
	rm -rf $(TMP_DIR)opencv
	mkdir $(TMP_DIR)opencv
	cd $(TMP_DIR)opencv
	curl -Lo opencv.zip https://github.com/opencv/opencv/archive/$(OPENCV_VERSION).zip
	unzip -q opencv.zip
	curl -Lo opencv_contrib.zip https://github.com/opencv/opencv_contrib/archive/$(OPENCV_VERSION).zip
	unzip -q opencv_contrib.zip
	rm opencv.zip opencv_contrib.zip
	cd -

# Build OpenCV.
build:
	cd $(TMP_DIR)opencv/opencv-$(OPENCV_VERSION)
	mkdir build
	cd build
	rm -rf *
	cmake -D CMAKE_BUILD_TYPE=RELEASE -D CMAKE_INSTALL_PREFIX=/usr/local -D BUILD_SHARED_LIBS=${BUILD_SHARED_LIBS} -D OPENCV_EXTRA_MODULES_PATH=$(TMP_DIR)opencv/opencv_contrib-$(OPENCV_VERSION)/modules -D BUILD_DOCS=OFF -D BUILD_EXAMPLES=OFF -D BUILD_TESTS=OFF -D BUILD_PERF_TESTS=OFF -D BUILD_opencv_java=NO -D BUILD_opencv_python=NO -D BUILD_opencv_python2=NO -D BUILD_opencv_python3=NO -D WITH_JASPER=OFF -DOPENCV_GENERATE_PKGCONFIG=ON ..
	$(MAKE) -j $(shell nproc --all)
	$(MAKE) preinstall
	cd -

# Build OpenCV for NVidia Jetson with CUDA.
build_jetson:
	cd $(TMP_DIR)opencv/opencv-$(OPENCV_VERSION)
	mkdir build
	cd build
	rm -rf *
	cmake -D CMAKE_BUILD_TYPE=RELEASE \
		-D CMAKE_INSTALL_PREFIX=/usr/local \
		-D EIGEN_INCLUDE_PATH=/usr/include/eigen3 \
		-D BUILD_SHARED_LIBS=${BUILD_SHARED_LIBS} \
		-D OPENCV_EXTRA_MODULES_PATH=$(TMP_DIR)opencv/opencv_contrib-$(OPENCV_VERSION)/modules \
		-D BUILD_DOCS=OFF -D BUILD_EXAMPLES=OFF -D BUILD_TESTS=OFF -D BUILD_PERF_TESTS=OFF -D BUILD_opencv_java=OFF -D BUILD_opencv_python=NO -D BUILD_opencv_python2=NO -D BUILD_opencv_python3=NO \
		-D WITH_JASPER=OFF \
        -D WITH_OPENCL=OFF \
		-D WITH_CUDA=ON \
		-D CUDA_ARCH_BIN="5.3,6.2,7.2" \
		-D CUDA_ARCH_PTX="" \
		-D WITH_CUDNN=ON \
		-D WITH_CUBLAS=1 \
		-D ENABLE_FAST_MATH=1 \
		-D CUDA_FAST_MATH=1 \
		-D OPENCV_DNN_CUDA=ON \
		-D ENABLE_NEON=ON \
		-D WITH_QT=OFF \
		-D WITH_OPENMP=ON \
		-D WITH_OPENGL=ON \
		-D BUILD_TIFF=ON \
		-D WITH_FFMPEG=ON \
		-D WITH_GSTREAMER=ON \
		-D WITH_TBB=ON \
		-D BUILD_TBB=ON \
		-D BUILD_TESTS=OFF \
		-D WITH_EIGEN=ON \
		-D WITH_V4L=ON \
		-D WITH_LIBV4L=ON \
        -D CUDA_TOOLKIT_ROOT_DIR=/usr/local/cuda/ \
		-D OPENCV_GENERATE_PKGCONFIG=ON ..
	$(MAKE) -j $(shell nproc --all)
	$(MAKE) preinstall
	cd -

# Build OpenCV with cuda.
build_cuda:
	cd $(TMP_DIR)opencv/opencv-$(OPENCV_VERSION)
	mkdir build
	cd build
	rm -rf *
	cmake -j $(shell nproc --all) -D CMAKE_BUILD_TYPE=RELEASE -D CMAKE_INSTALL_PREFIX=/usr/local -D BUILD_SHARED_LIBS=${BUILD_SHARED_LIBS} -D OPENCV_EXTRA_MODULES_PATH=$(TMP_DIR)opencv/opencv_contrib-$(OPENCV_VERSION)/modules -D WITH_FFMPEG=ON -D BUILD_DOCS=OFF -D BUILD_EXAMPLES=OFF -D BUILD_TESTS=OFF -D BUILD_PERF_TESTS=OFF -D BUILD_opencv_java=NO -D BUILD_opencv_python=NO -D BUILD_opencv_python2=NO -D BUILD_opencv_python3=NO -D WITH_JASPER=OFF -DOPENCV_GENERATE_PKGCONFIG=ON -DWITH_CUDA=ON -DENABLE_FAST_MATH=1 -DCUDA_FAST_MATH=1 -DWITH_CUBLAS=1 -DCUDA_TOOLKIT_ROOT_DIR=/usr/local/cuda/ -DBUILD_opencv_cudacodec=OFF -D WITH_CUDNN=ON -D OPENCV_DNN_CUDA=ON -D CUDA_GENERATION=Auto ..
	$(MAKE) -j $(shell nproc --all)
	$(MAKE) preinstall
	cd -

# Build OpenCV staticly linked
build_static:
	cd $(TMP_DIR)opencv/opencv-$(OPENCV_VERSION)
	mkdir build
	cd build
	rm -rf *
	cmake -D CMAKE_BUILD_TYPE=RELEASE -D CMAKE_INSTALL_PREFIX=/usr/local -D BUILD_SHARED_LIBS=OFF -D OPENCV_EXTRA_MODULES_PATH=$(TMP_DIR)opencv/opencv_contrib-$(OPENCV_VERSION)/modules -D BUILD_DOCS=OFF -D BUILD_EXAMPLES=OFF -D BUILD_TESTS=OFF -D BUILD_PERF_TESTS=OFF -D BUILD_opencv_java=NO -D BUILD_opencv_python=NO -D BUILD_opencv_python2=NO -D BUILD_opencv_python3=NO -DWITH_JASPER=OFF -DWITH_QT=OFF -DWITH_GTK=OFF -DWITH_FFMPEG=OFF -DWITH_TIFF=OFF -DWITH_WEBP=OFF -DWITH_PNG=OFF -DWITH_1394=OFF -DWITH_OPENJPEG=OFF -DOPENCV_GENERATE_PKGCONFIG=ON ..
	$(MAKE) -j $(shell nproc --all)
	$(MAKE) preinstall
	cd -

# Build OpenCV with cuda.
build_all:
	cd $(TMP_DIR)opencv/opencv-$(OPENCV_VERSION)
	mkdir build
	cd build
	rm -rf *
	cmake -j $(shell nproc --all) -D CMAKE_BUILD_TYPE=RELEASE -D CMAKE_INSTALL_PREFIX=/usr/local -D BUILD_SHARED_LIBS=${BUILD_SHARED_LIBS} -D ENABLE_CXX11=ON -D OPENCV_EXTRA_MODULES_PATH=$(TMP_DIR)opencv/opencv_contrib-$(OPENCV_VERSION)/modules -D WITH_INF_ENGINE=ON -D InferenceEngine_DIR=/usr/local/dldt/inference-engine/build -D BUILD_DOCS=OFF -D BUILD_EXAMPLES=OFF -D BUILD_TESTS=OFF -D BUILD_PERF_TESTS=OFF -D BUILD_opencv_java=NO -D BUILD_opencv_python=NO -D BUILD_opencv_python2=NO -D BUILD_opencv_python3=NO -D WITH_JASPER=OFF -DOPENCV_GENERATE_PKGCONFIG=ON -DWITH_CUDA=ON -DENABLE_FAST_MATH=1 -DCUDA_FAST_MATH=1 -DWITH_CUBLAS=1 -DCUDA_TOOLKIT_ROOT_DIR=/usr/local/cuda/ -DBUILD_opencv_cudacodec=OFF -D WITH_CUDNN=ON -D OPENCV_DNN_CUDA=ON -D CUDA_GENERATION=Auto ..
	$(MAKE) -j $(shell nproc --all)
	$(MAKE) preinstall
	cd -

# Cleanup temporary build files.
clean:
	rm -rf $(TMP_DIR)opencv

# Cleanup old library files.
sudo_pre_install_clean:
	sudo rm -rf /usr/local/lib/cmake/opencv4/
	sudo rm -rf /usr/local/lib/libopencv*
	sudo rm -rf /usr/local/lib/pkgconfig/opencv*
	sudo rm -rf /usr/local/include/opencv*

# Do everything.
install: deps download sudo_pre_install_clean build sudo_install clean

# Do everything on Jetson.
install_jetson: deps download build_jetson sudo_install clean

# Do everything with cuda.
install_cuda: deps download sudo_pre_install_clean build_cuda sudo_install clean

# Do everything statically.
install_static: deps download sudo_pre_install_clean build_static sudo_install clean

# Install system wide.
sudo_install:
	cd $(TMP_DIR)opencv/opencv-$(OPENCV_VERSION)/build
	sudo $(MAKE) install
	sudo ldconfig
	cd -