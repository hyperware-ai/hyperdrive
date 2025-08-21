import React, { useState, useEffect, useMemo } from 'react';
import useAppsStore from "../store/appStoreStore";

interface PackageSelectorProps {
    onPackageSelect: (packageName: string, publisherId: string) => void;
    publisherId: string;
}

const PackageSelector: React.FC<PackageSelectorProps> = ({
    onPackageSelect,
    publisherId
}) => {
    const { installed, fetchInstalled } = useAppsStore();
    const [selectedPackage, setSelectedPackage] = useState<string>("");

    useEffect(() => {
        fetchInstalled();
    }, []);

    const filteredPackages = useMemo(() =>
        Object.keys(installed).filter(packageFullName =>
            packageFullName.endsWith(publisherId)
        ), [installed, publisherId]
    );

    useEffect(() => {
        if (selectedPackage) {
            const packageName = selectedPackage.split(':')[0];
            onPackageSelect(packageName, publisherId);
        }
    }, [selectedPackage, onPackageSelect, publisherId]);

    useEffect(() => {
        if (filteredPackages.length > 0 && !selectedPackage) {
            setSelectedPackage(filteredPackages[0]);
        }
    }, [filteredPackages, selectedPackage]);

    const handlePackageChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
        setSelectedPackage(e.target.value);
    };

    return (
        <select
            value={selectedPackage}
            onChange={handlePackageChange}
            className="self-stretch outline-2 outline-offset-2 outline-iris/10 dark:outline-neon/10 rounded"
        >
            {filteredPackages.length === 0 ? (
                <option disabled value="">
                    No apps installed with publisher {publisherId}
                </option>
            ) : (
                filteredPackages.map(packageFullName => (
                    <option key={packageFullName} value={packageFullName}>
                        {packageFullName}
                    </option>
                ))
            )}
        </select>
    );
};

export default PackageSelector;